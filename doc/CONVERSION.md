### Migration from Szurubooru

#### THIS GUIDE IS A WORK-IN-PROGRESS

This guide assumes you have set up an Oxibooru instance already.

1. Create a dump of the Szurubooru database

    Navigate to the Szurubooru source directory and run.
    ```console
    user@host:szuru$ docker-compose up -d sql
    user@host:szuru$ docker exec szuru-sql-1 pg_dump -U ${SZURU_POSTGRES_USER} --no-owner --no-privileges szuru > backup.sql
    ```
    
2. Restore Szurubooru database
    
    Navigate to the Oxibooru source directory and run.
    ```console
    user@host:oxibooru$ docker-compose up -d sql
    ```
    Now create szuru schema in Oxibooru database and use backup.sql to restore it.

    ```console
    user@host:oxibooru$ docker exec oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi -c "ALTER SCHEMA public RENAME TO oxi;"
    user@host:oxibooru$ docker exec oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi -c "CREATE SCHEMA public";
    user@host:oxibooru$ cat backup.sql | docker exec -i oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi
    ```
    
3. Run conversion script
    ```console
    user@host:oxibooru$ cat scripts/convert_szuru_database/up.sql | docker exec -i oxibooru-sql-1 psql -U postgres -d oxi --single-transaction
    ```
    Any errors encountered will rollback the conversion of the database. If you would like to opt-out of this
    behavior and attempt a partial conversion, you can omit the --single-transaction argument.

4. Run some additional commands

    Now we need to convert the data folder, post checksums, and post signatures.
    This can be done via Oxibooru's admin cli. First, spin up the Oxibooru containers:
    ```console
    user@host:oxibooru$ docker-compose up -d
    ```
    Then, enter admin cli:
    ```console
    user@host:oxibooru$ docker exec -it oxibooru-server-1 ./server --admin
    ```
    Execute these tasks in order: rename_post_content, recompute_post_checksums, recompute_post_signatures