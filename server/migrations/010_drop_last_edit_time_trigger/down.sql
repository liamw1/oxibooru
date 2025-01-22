CREATE FUNCTION diesel_manage_last_edit_time(_tbl regclass) RETURNS VOID AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_last_edit_time BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE diesel_set_last_edit_time()', _tbl);
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION diesel_set_last_edit_time() RETURNS TRIGGER AS $$
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

SELECT diesel_manage_last_edit_time('user');
SELECT diesel_manage_last_edit_time('user_token');
SELECT diesel_manage_last_edit_time('tag_category');
SELECT diesel_manage_last_edit_time('tag');
SELECT diesel_manage_last_edit_time('post');
SELECT diesel_manage_last_edit_time('comment');
SELECT diesel_manage_last_edit_time('pool_category');
SELECT diesel_manage_last_edit_time('pool');