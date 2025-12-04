# Converting from Szurubooru

## Before Starting

All software is fallible, especially software that hasn't been extensively tested. Please make a back up of your database before attempting a conversion.

This guide assumes you have set up an Oxibooru instance already. See the [installation instructions](INSTALL.md) for more details. It also assumes the Oxibooru database is _empty_, which is the case on an initial install. Make sure that you run the server container at least once to make sure it has been installed correctly and that the migrations have been applied.

The guide doesn't cover migrating the `config.yaml`, but the structure of the Oxibooru equivalent, `config.toml`, is almost identical. Just copy over the settings from the config.yaml manually if you want them.

If you encounter any issues during the conversion process, please open up an issue on [Github](https://github.com/liamw1/oxibooru/issues).

## Known Limitations

Some aspects of a Szurubooru instance can't be converted to an Oxibooru instance. Depending on how you're using your database, these limitations may make total migration difficult or impossible.

1. **Passwords**

    Password hashing is done a bit differently in Oxibooru, so this unfortunately means that passwords can't be migrated over at the moment. Passwords can be reset individually via the admin cli or can be reset via reset requests if SMTP information is provided in the `config.toml`.

2. **Some image formats**

    Currently, Oxibooru doesn't support HEIF or HEIC file formats. It's possible that they will be supported in the future if I can find good crates for them.

3. **Some post types**

    Oxibooru does not currently support YouTube posts and is unlikely to support them in the future.

## Let's begin

If you're able to accept these limitations, let's start converting...

1. **Create a dump of the Szurubooru database**

    Navigate to the Szurubooru source directory and run

    ```console
    docker compose up -d sql
    docker exec szuru-sql-1 pg_dump -U SZURU_POSTGRES_USER --no-owner --no-privileges szuru > backup.sql
    ```

    where `SZURU_POSTGRES_USER` is the value of the `POSTGRES_USER` environment variable defined in the Szurubooru `.env` file.

2. **Restore Szurubooru database**

    Navigate to the Oxibooru source directory and run

    ```console
    docker compose up -d sql
    ```

    Now move backup.sql to the Oxibooru directory, create a szuru schema in the Oxibooru database and restore the Szurubooru database.

    ```console
    docker exec oxibooru-sql-1 psql -U POSTGRES_USER -d POSTGRES_DB -c "ALTER SCHEMA public RENAME TO POSTGRES_DB;"
    docker exec oxibooru-sql-1 psql -U POSTGRES_USER -d POSTGRES_DB -c "CREATE SCHEMA public;"
    cat backup.sql | docker exec -i oxibooru-sql-1 psql -U POSTGRES_USER -d POSTGRES_DB
    ```

    Here `POSTGRES_USER` and `POSTGRES_DB` are the values of the environment variables defined in the Oxibooru `.env` file.

3. **Run conversion script**
    Some Szurubooru tables contain serialized python objects that need to be unserialized to perform the conversion. Because of this, we must enable the PL/Python procedural language in PostgreSQL. To do this, run

    ```console
    docker exec -i oxibooru-sql-1 bash -s < scripts/install_plpython3u.sh
    ```

    Now, we can run the conversion script:

    ```console
    cat scripts/convert_szuru_database.sql | docker exec -i oxibooru-sql-1 psql -U POSTGRES_USER -d POSTGRES_DB --single-transaction
    ```

    Any errors encountered will rollback the conversion of the database. If you would like to opt-out of this behavior and attempt a partial conversion, you can omit the `--single-transaction` flag.

    Pool and tag names are unique and case insensitive in Oxibooru, so "tag" and "Tag" can't coexist in the database. In Szurubooru, it is possible that these can coexist, although it generally tries to prevent it. In the event that your Szurubooru database contains names which only differ by case, those names will be modified to `{name}_name_modified_{tag_id}_{order}` to prevent conflicts. You can search for the affected tag/pool names by entering `*_name_modified_*` in the tag/pool search bar.

4. **Rename data files**

    Oxibooru uses a different file naming convention, so these will need to be recomputed. First, if you have any custom-thumbnails, you should move the custom-thumbnails folder from `MOUNT_DATA/posts/custom-thumbnails` to `MOUNT_DATA/custom-thumbnails`.

    Next, make sure the `MOUNT_DATA` environment variable is pointed towards the data directory of the Szurubooru database. This step will modify the filenames of posts and thumbnails, making Szurubooru unable to read them. This should be reversible using the Szurubooru admin function `reset_filenames()`, but I haven't tested that so you may want to make a backup.

    Now, spin up the Oxibooru containers:

    ```console
    docker compose up -d
    ```

    Now, enter Oxibooru's admin cli:

    ```console
    docker exec -it oxibooru-server-1 ./server --admin
    ```

    From there, run the reset_filenames command, which will automatically rename every file in the data directory to match the Oxibooru convention.

    ```console
    Please select a task: reset_filenames
    ```

5. **Compute thumbnail sizes**

    Oxibooru stores thumbnail sizes along with file sizes so it can quickly compute the size of the database. To store sizes of existing thumbnails, run `reset_thumbnail_sizes` in the admin cli:

    ```console
    Please select a task: reset_thumbnail_sizes
    ```

    This step is optional. The only effect skipping it would have is that the disk usage that the client displays will be slightly inaccurate.

6. **Recompute post properties**

    Oxibooru also uses a different checksum and image signature algorithm, so these will need to be recomputed. These can both be accomplished via the admin cli.

    Run `recompute_post_checksums` and `recompute_post_signatures` in the admin cli. For databases with many posts, these functions can take a bit of time to complete, so be prepared for that. `recompute_post_checksums` runs at around 240 posts/sec and `recompute_post_signatures` runs at around 150 posts/sec on my machine.

    ```console
    Please select a task: recompute_post_checksums
    ```

    ```console
    Please select a task: recompute_post_signatures
    ```

7. **Reset user passwords**

    At this point the database is almost completely converted to a proper Oxibooru instance. However, there are still some important things not covered by the previous steps. Currently, users won't be able to login using their original passwords. This is because of differences in how Oxibooru hashes and salts user passwords. At the moment, the only way to recover these users is to reset their passwords. For personal databases with a small number of users, this can be done via the admin cli fairly easily.

    Upon entering `reset_password` in the admin cli, you will be prompted for the username whose password you want to reset and the new password you would like for that user.

    ```console
    Please select a task: reset_password
    ```

8. **We're done!**
