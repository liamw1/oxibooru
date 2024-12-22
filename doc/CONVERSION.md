## Converting from Szurubooru

### Before Starting
All software is fallible, especially software that hasn't been extensively
tested. Please make a back up of your database before attempting a conversion.

This guide assumes you have set up an Oxibooru instance already. See the 
[installation instructions](INSTALL.md) for more details. It also assumes
the Oxibooru database is _empty_, which is the case on an initial install.

The guide doesn't cover migrating the config.yaml, but the structure of
the Oxibooru equivalent, config.toml, is almost identical. Just copy over the
settings from the config.yaml manually if you want them.

If you encounter any issues during the conversion process, please open up an
issue on [Github](https://github.com/liamw1/oxibooru/issues).

### Known Limitations
Some aspects of a Szurubooru instance can't be converted to an Oxibooru
instance. Depending on how you're using your database, these limitations may
make total migration difficult or impossible. In order of decreasing severity,
these are:

1. **Passwords**

    Password hashing is done a bit differently in Oxibooru,
    so this unfortunately means that passwords can't be migrated over at the
    moment. Passwords can be reset individually via the admin cli, but this is
    impractical and insecure for databases with many users. If there are any 
    such databases out there in production, it may be best to stick with
    Szurubooru for the forseeable future.
    
2. **Some image formats**
    
    Currently, Oxibooru doesn't support AVIF, HEIF, HEIC, or SWF file formats.
    In the future, support for AVIF is very likely. HEIF and HEIC are a little
    less likely and depends on if I can find a good crate for them. SWF is 
    probably never going to be supported.

3. **Snapshots**

    Snapshots are not implemented in Oxibooru, and I don't have plans to
    implement them unless someone gives me a compelling case for them. I'm still
    not sure what purpose they actually served in Szurubooru. If you're a big
    snapshot fan then you may want to hold off on migrating.

4. **Post Notes**
    
    This one isn't a fundamental limitation. I just don't know enough Postgres
    to convert BYTEA to REAL[2][] so the conversion script ignores post notes. 
    I'll get around to fixing this at some point.

### Let's begin
1. **Create a dump of the Szurubooru database**

    Navigate to the Szurubooru source directory and run.
    ```console
    user@host:szuru$ docker-compose up -d sql
    user@host:szuru$ docker exec szuru-sql-1 pg_dump -U ${SZURU_POSTGRES_USER} --no-owner --no-privileges szuru > backup.sql
    ```
    
2. **Restore Szurubooru database**
    
    Navigate to the Oxibooru source directory and run.
    ```console
    user@host:oxibooru$ docker-compose up -d sql
    ```
    Now create szuru schema in Oxibooru database and use backup.sql to restore 
    it.

    ```console
    user@host:oxibooru$ docker exec oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi -c "ALTER SCHEMA public RENAME TO oxi;"
    user@host:oxibooru$ docker exec oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi -c "CREATE SCHEMA public;"
    user@host:oxibooru$ cat backup.sql | docker exec -i oxibooru-sql-1 psql -U ${POSTGRES_USER} -d oxi
    ```
    
3. **Run conversion script**
    ```console
    user@host:oxibooru$ cat scripts/convert_szuru_database/up.sql | docker exec -i oxibooru-sql-1 psql -U postgres -d oxi --single-transaction
    ```
    Any errors encountered will rollback the conversion of the database. If you 
    would like to opt-out of this behavior and attempt a partial conversion, you
    can omit the --single-transaction argument.

4. **Rename data files**

    Oxibooru uses a different file naming convention, so these will need to be 
    recomputed. First, if you have any custom-thumbnails, you should move the
    custom-thumbnails folder from $\{MOUNT_DATA\}/posts/custom-thumbnails to
    $\{MOUNT_DATA\}/custom-thumbnails. Then, spin up the Oxibooru containers:
    ```console
    user@host:oxibooru$ docker-compose up -d
    ```
    Now, enter Oxibooru's admin cli:
    ```console
    user@host:oxibooru$ docker exec -it oxibooru-server-1 ./server --admin
    ```
    From there, run the rename_data_paths command, which will automatically
    rename every file in the data directory to match the Oxibooru convention.

5. **Recompute post properties**

    Oxibooru also uses a different checksum and image signature algorithm, so 
    these will need to be recomputed. These can both be accomplished via the 
    admin cli.
    
    Run recompute_post_checksums and recompute_post_signatures in the admin
    cli. For databases with many posts recompute_post_signatures can take quite 
    a bit of time, so be prepared for that.
    
6. **Reset user passwords**

    At this point the database is almost completely converted to a proper
    Oxibooru instance. However, there are still some important things not
    covered by the previous steps. Currently, users won't be able to login using
    their original passwords. This is because of differences in how Oxibooru 
    hashes and salts user passwords. At the moment, the only way to recover 
    these users is to reset their passwords. For personal databases with a small
    number of users, this can be done via the admin cli fairly easily.
    
    Upon entering reset_password in the admin cli, you will be prompted for the
    username whose password you want to reset and the new password you would 
    like for that user.
    
7. **We're done!**