-- This file was automatically created by Diesel to setup helper functions
-- and other internal bookkeeping. This file is safe to edit, any future
-- changes will be added to existing projects as new migrations.




-- Sets up a trigger for the given table to automatically set a column called
-- `last_edit_time` whenever the row is modified (unless `last_edit_time` was included
-- in the modified columns)
--
-- # Example
--
-- ```sql
-- CREATE TABLE users (id SERIAL PRIMARY KEY, last_edit_time TIMESTAMP NOT NULL DEFAULT NOW());
--
-- SELECT diesel_manage_last_edit_time('users');
-- ```
CREATE OR REPLACE FUNCTION diesel_manage_last_edit_time(_tbl regclass) RETURNS VOID AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_last_edit_time BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE diesel_set_last_edit_time()', _tbl);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION diesel_set_last_edit_time() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.last_edit_time IS NOT DISTINCT FROM OLD.last_edit_time
    ) THEN
        NEW.last_edit_time := CURRENT_TIMESTAMP;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
