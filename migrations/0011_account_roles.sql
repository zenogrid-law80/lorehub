ALTER TABLE users
    ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
    CHECK (role IN ('user', 'admin'));

UPDATE users
SET role = 'admin'
WHERE id = (
    SELECT id
    FROM users
    ORDER BY created_at, id
    LIMIT 1
);

CREATE FUNCTION assign_initial_admin() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(1196577879);
    IF NOT EXISTS (SELECT 1 FROM users WHERE role = 'admin') THEN
        NEW.role = 'admin';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER users_assign_initial_admin
    BEFORE INSERT ON users
    FOR EACH ROW EXECUTE FUNCTION assign_initial_admin();
