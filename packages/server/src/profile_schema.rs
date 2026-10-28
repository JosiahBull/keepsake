//! Register the explicitly scoped equivalents of the sole-profile compatibility routes.
use utoipa::{
    Modify, PartialSchema,
    openapi::{
        Content, OpenApi, Ref, Required,
        path::{ParameterBuilder, ParameterIn},
        response::ResponseBuilder,
    },
};

pub struct ProfilePaths;

impl Modify for ProfilePaths {
    fn modify(&self, api: &mut OpenApi) {
        let paths = api.paths.paths.clone();
        for (path, mut item) in paths {
            if ![
                "/api/facts",
                "/api/practice",
                "/api/reviews",
                "/api/study/",
                "/api/media",
            ]
            .iter()
            .any(|prefix| path.starts_with(prefix))
            {
                continue;
            }
            item.parameters.get_or_insert_default().push(
                ParameterBuilder::new()
                    .name("profile_id")
                    .parameter_in(ParameterIn::Path)
                    .required(Required::True)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            for operation in [
                &mut item.get,
                &mut item.post,
                &mut item.put,
                &mut item.patch,
                &mut item.delete,
                &mut item.head,
                &mut item.options,
                &mut item.trace,
            ]
            .into_iter()
            .flatten()
            {
                if let Some(id) = &mut operation.operation_id {
                    *id = format!("profile_{id}");
                }
                for (status, description) in [
                    ("400", "Invalid profile or request"),
                    ("404", "Profile or item not found"),
                ] {
                    operation
                        .responses
                        .responses
                        .entry(status.into())
                        .or_insert_with(|| {
                            ResponseBuilder::new()
                                .description(description)
                                .content(
                                    "application/json",
                                    Content::new(Some(Ref::from_schema_name("ErrorBody"))),
                                )
                                .build()
                                .into()
                        });
                }
            }
            api.paths
                .paths
                .insert(format!("/api/profiles/{{profile_id}}{}", &path[4..]), item);
        }
    }
}
