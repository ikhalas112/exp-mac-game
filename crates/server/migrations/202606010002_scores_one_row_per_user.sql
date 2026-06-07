DELETE FROM scores
WHERE id NOT IN (
    SELECT DISTINCT ON (user_id) id
    FROM scores
    ORDER BY user_id, score DESC, created_at ASC
);

CREATE UNIQUE INDEX IF NOT EXISTS scores_user_id_unique_idx
    ON scores (user_id);
