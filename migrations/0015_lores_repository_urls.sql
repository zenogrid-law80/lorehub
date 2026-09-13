UPDATE pipelines
SET repository_url = 'lores://' || substring(repository_url FROM 8)
WHERE repository_url LIKE 'lore://%';

UPDATE ci_pipeline_routes
SET repository_url = 'lores://' || substring(repository_url FROM 8)
WHERE repository_url LIKE 'lore://%';
