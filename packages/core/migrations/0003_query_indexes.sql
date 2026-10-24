CREATE INDEX facts_matching_prompt_answer
ON facts (lower(trim(prompt)), lower(trim(answer)));
CREATE INDEX facts_matching_answer
ON facts (lower(trim(answer)), id);
CREATE INDEX reviews_fact_id
ON reviews (fact_id, id);
