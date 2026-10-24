use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};
use keepsake_cli::{
    agent::{self, Agent},
    source,
};
use keepsake_core::{
    backup, db,
    facts::{self, Fact, NewFact},
    paging,
};
use std::{io::Read, path::PathBuf};

const AFTER_HELP: &str = concat!(
    "Examples:\n",
    "  keepsake add --prompt 'What does WAL stand for?' --answer 'Write-ahead logging' --tag sqlite\n",
    "  keepsake add --prompt 'What does WAL stand for?' --answer 'Write-ahead logging' --agent claude --dry-run\n",
    "  keepsake add --url https://example.com/article --prompt 'Remember the main ideas' --dry-run\n",
    "  keepsake generate 1 --agent codex --dry-run\n",
    "  keepsake add --file reviewed-fact.json\n",
    "  keepsake --json list\n",
    "\n",
    "Agent calls send the supplied fact/context to your chosen provider using your local CLI login.\n",
    "Review generated drafts: use --dry-run before importing with add --file. No agent is called for manual facts by default; --url uses Claude Haiku unless --agent is supplied.",
);

#[derive(Parser)]
#[command(
    name = "keepsake",
    version,
    about = "Capture facts and draft varied retrieval practice",
    after_help = AFTER_HELP
)]
struct Cli {
    /// SQLite URL. Defaults to DATABASE_URL or data/keepsake.db in the current directory.
    #[arg(long, global = true)]
    database: Option<String>,

    /// Emit JSON (dry-run always emits JSON).
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Save manual facts or extract them from a URL.
    Add(Add),

    /// List one page of facts, newest first.
    List {
        #[arg(long, default_value_t = 25, value_parser = clap::value_parser!(i64).range(1..=100))]
        limit: i64,
        /// Continue with the next_cursor returned by the preceding page.
        #[arg(long, value_parser = clap::value_parser!(i64).range(1..))]
        before: Option<i64>,
        /// Literal substring search across question, answer, context, and tags.
        #[arg(long, default_value = "")]
        search: String,
    },

    /// Show a saved fact and its practice variants.
    Show { id: i64 },

    /// Draft additional practice for an existing fact.
    Generate {
        id: i64,
        #[command(flatten)]
        agent: AgentOptions,

        /// Save reviewed JSON from generate --dry-run (original fact must match).
        #[arg(long, conflicts_with = "agent")]
        file: Option<PathBuf>,

        #[arg(long)]
        dry_run: bool,
    },

    /// Describe supported practice methods and study activities.
    Methods,

    /// Save a consistent SQLite snapshot without changing the source.
    Backup { destination: PathBuf },

    /// Restore a backup into a new file, without running migrations.
    Restore {
        source: PathBuf,
        #[arg(long)]
        to: PathBuf,
    },
}

#[derive(Args)]
struct Add {
    /// Retrieval question, or learning instructions when used with --url.
    #[arg(long, required_unless_present = "file", conflicts_with = "file")]
    prompt: Option<String>,
    /// Authoritative answer, preserved exactly when AI drafts variants.
    #[arg(long, required_unless_present_any = ["file", "url"], conflicts_with_all = ["file", "url"])]
    answer: Option<String>,
    #[arg(long, default_value = "", conflicts_with_all = ["file", "url"])]
    context: String,
    #[arg(long, default_value = "", conflicts_with_all = ["file", "url"])]
    source: String,
    #[arg(long = "tag", conflicts_with = "file")]
    tags: Vec<String>,
    /// Fetch a page and extract facts following --prompt. Defaults to Claude Haiku.
    #[arg(long, requires = "prompt", conflicts_with = "file")]
    url: Option<String>,
    /// Maximum facts extracted from a URL (default 5, maximum 20).
    #[arg(long, requires = "url", value_parser = clap::value_parser!(u32).range(1..=20))]
    limit: Option<u32>,
    /// Import one NewFact or an array; '-' reads standard input (max 1 MiB).
    #[arg(long)]
    file: Option<PathBuf>,
    #[command(flatten)]
    agent: AgentOptions,
    /// Print proposed facts as JSON without saving.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct AgentOptions {
    /// Optional external CLI: Claude Haiku or Codex Luna by default.
    #[arg(long, value_enum)]
    agent: Option<Agent>,
    /// Override the provider model (requires --agent).
    #[arg(long, requires = "agent")]
    model: Option<String>,
    /// External CLI executable path (requires --agent).
    #[arg(long, requires = "agent")]
    agent_command: Option<PathBuf>,
    /// Agent deadline, in seconds (1–600).
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=600))]
    timeout_seconds: u64,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        database: database_url,
        json,
        command,
    } = cli;

    match command {
        Commands::Methods => show_methods(json),
        Commands::Add(add) => add_facts(add, database_url, json).await,
        Commands::Backup { destination } => output_snapshot(
            backup::create(&database(database_url), &destination).await?,
            json,
        ),
        Commands::Restore { source, to } => {
            output_snapshot(backup::restore(&source, &to).await?, json)
        }
        command => {
            let pool = db::connect(&database(database_url)).await?;

            match command {
                Commands::List {
                    limit,
                    before,
                    search,
                } => {
                    let saved = paging::facts(
                        &pool,
                        &paging::FactQuery {
                            limit,
                            before_id: before,
                            search,
                        },
                    )
                    .await?;
                    if json {
                        print_json(&saved)?;
                    } else if saved.items.is_empty() {
                        println!("No facts found.");
                    } else {
                        for fact in &saved.items {
                            println!("{}\t{}", fact.id, fact.prompt);
                        }
                        if let Some(cursor) = saved.next_cursor {
                            println!("\nContinue with --before {cursor} and the same search.");
                        }
                    }
                }
                Commands::Show { id } => {
                    let fact = facts::get(&pool, id).await?.context("fact not found")?;
                    output_fact(fact, json)?;
                }
                Commands::Generate {
                    id,
                    agent,
                    file,
                    dry_run,
                } => {
                    let fact = facts::get(&pool, id).await?.context("fact not found")?;
                    let draft = draft_practices(fact, agent, file).await?;
                    if dry_run {
                        print_json(&draft)?;
                    } else {
                        let saved = facts::add_practices(&pool, id, draft.practices).await?;
                        output_fact(saved, json)?;
                    }
                }
                Commands::Add(_)
                | Commands::Methods
                | Commands::Backup { .. }
                | Commands::Restore { .. } => unreachable!(),
            }
            Ok(())
        }
    }
}

fn output_snapshot(snapshot: backup::Snapshot, json: bool) -> anyhow::Result<()> {
    if json {
        print_json(&snapshot)?;
    } else {
        println!(
            "{} ({} facts, {} practices, {} reviews)",
            snapshot.path.display(),
            snapshot.facts,
            snapshot.practices,
            snapshot.reviews
        );
    }
    Ok(())
}

fn show_methods(json: bool) -> anyhow::Result<()> {
    let methods = serde_json::json!([
        {
            "kind": "flashcard",
            "activity": "Recall before revealing the answer"
        },
        {
            "kind": "multiple_choice",
            "activity": "Choose one answer, then read corrective feedback"
        },
        {
            "kind": "cloze",
            "activity": "Supply a missing phrase from a meaningful cue"
        },
        {
            "kind": "explain",
            "activity": "Explain a concept against essential points"
        },
        {
            "kind": "apply",
            "activity": "Use knowledge in a scenario"
        },
        {
            "kind": "matching",
            "activity": "Mini game matching cues to answers across your library"
        }
    ]);

    if json {
        print_json(&methods)?;
    } else {
        for method in methods.as_array().expect("methods array") {
            println!(
                "{} — {}",
                method["kind"].as_str().unwrap(),
                method["activity"].as_str().unwrap()
            );
        }
        println!(
            "\nResearch: docs/research/learning-and-retention-2026-10-24.md\nVariety is a design proposal; retrieval, corrective feedback and spacing provide the learning foundation."
        );
    }
    Ok(())
}

async fn add_facts(add: Add, database_url: Option<String>, json: bool) -> anyhow::Result<()> {
    // Complete fetching, generation and validation before touching the database.
    let is_url = add.url.is_some();
    let (mut proposed, batch) = if let Some(url) = add.url {
        let instruction = add.prompt.as_deref().unwrap_or_default();
        if instruction.trim().is_empty() || instruction.len() > 8000 {
            bail!("URL prompt must contain 1 to 8000 bytes");
        }
        let document = source::fetch(&url).await?;
        let request = agent::Extraction {
            document: &document,
            instruction,
            limit: add.limit.unwrap_or(5) as usize,
            tags: &add.tags,
        };
        (
            agent::extract_facts(
                &request,
                add.agent.agent.unwrap_or(Agent::Claude),
                add.agent.model.as_deref(),
                add.agent.agent_command.as_deref(),
                add.agent.timeout_seconds,
            )
            .await?,
            true,
        )
    } else if let Some(path) = add.file {
        read_input(path)?
    } else {
        (
            vec![NewFact {
                prompt: add.prompt.unwrap_or_default(),
                answer: add.answer.unwrap_or_default(),
                context: add.context,
                source: add.source,
                tags: add.tags,
                practices: vec![],
            }],
            false,
        )
    };

    if proposed.is_empty() || proposed.len() > 20 {
        bail!("provide between 1 and 20 facts");
    }

    for fact in &mut proposed {
        facts::validate_new_fact(fact)?;
        if !is_url && let Some(provider) = add.agent.agent {
            fact.practices.extend(
                agent::generate(
                    fact,
                    provider,
                    add.agent.model.as_deref(),
                    add.agent.agent_command.as_deref(),
                    add.agent.timeout_seconds,
                )
                .await?,
            );
        }
        facts::validate_new_fact(fact)?;
    }

    if add.dry_run {
        return if batch {
            print_json(&proposed)
        } else {
            print_json(&proposed[0])
        };
    }

    let pool = db::connect(&database(database_url)).await?;
    let mut saved = facts::create_many(&pool, proposed).await?;
    pool.close().await;

    if batch && json {
        return print_json(&saved);
    }
    if batch {
        let count = saved.len();
        for fact in saved {
            output_fact(fact, false)?;
        }
        println!("\nSaved {count} facts.");
        return Ok(());
    }
    output_fact(saved.pop().context("saved fact missing")?, json)
}

async fn draft_practices(
    fact: Fact,
    options: AgentOptions,
    file: Option<PathBuf>,
) -> anyhow::Result<NewFact> {
    let id = fact.id;
    let mut draft = NewFact {
        prompt: fact.prompt,
        answer: fact.answer,
        context: fact.context,
        source: fact.source,
        tags: fact.tags,
        practices: Vec::new(),
    };

    if let Some(path) = file {
        let reviewed = read_fact(path)?;
        facts::validate_new_fact(&reviewed)?;
        if reviewed.prompt != draft.prompt
            || reviewed.answer != draft.answer
            || reviewed.context != draft.context
            || reviewed.source != draft.source
            || reviewed.tags != draft.tags
        {
            bail!(
                "reviewed draft does not match the existing fact; use the JSON from generate {id} --dry-run"
            );
        }
        draft.practices = reviewed.practices;
    } else {
        let provider = options
            .agent
            .context("generate requires --agent claude|codex or --file reviewed.json")?;
        draft.practices = agent::generate(
            &draft,
            provider,
            options.model.as_deref(),
            options.agent_command.as_deref(),
            options.timeout_seconds,
        )
        .await?;
    }
    Ok(draft)
}

fn database(explicit: Option<String>) -> String {
    explicit
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "sqlite:data/keepsake.db".into())
}

fn read_fact(path: PathBuf) -> anyhow::Result<NewFact> {
    let (mut facts, batch) = read_input(path)?;
    if batch {
        bail!("generate --file expects the single fact object from generate --dry-run");
    }
    facts.pop().context("fact JSON was empty")
}

fn read_input(path: PathBuf) -> anyhow::Result<(Vec<NewFact>, bool)> {
    let reader: Box<dyn Read> = if path.as_os_str() == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).context("open fact JSON")?)
    };

    let mut bytes = Vec::new();
    reader.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        bail!("fact JSON exceeds 1 MiB");
    }

    let value: serde_json::Value = serde_json::from_slice(&bytes).context("parse fact JSON")?;
    if value.is_array() {
        Ok((
            serde_json::from_value(value).context("parse fact array")?,
            true,
        ))
    } else {
        Ok((
            vec![serde_json::from_value(value).context("parse fact")?],
            false,
        ))
    }
}

fn print_json(value: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn output_fact(fact: Fact, json: bool) -> anyhow::Result<()> {
    if json {
        print_json(&fact)
    } else {
        println!("#{} {}\n{}", fact.id, fact.prompt, fact.answer);
        if !fact.context.is_empty() {
            println!("\nContext: {}", fact.context);
        }
        if !fact.source.is_empty() {
            println!("Source: {}", fact.source);
        }
        if !fact.tags.is_empty() {
            println!("Tags: {}", fact.tags.join(", "));
        }
        for practice in fact.practices {
            println!(
                "\n{}: {}\nAnswer: {}",
                serde_json::to_value(practice.kind)?
                    .as_str()
                    .unwrap_or("practice"),
                practice.prompt,
                practice.answer
            );
            if !practice.choices.is_empty() {
                println!("Choices: {}", practice.choices.join(" | "));
            }
            if !practice.explanation.is_empty() {
                println!("Feedback: {}", practice.explanation);
            }
        }
        Ok(())
    }
}
