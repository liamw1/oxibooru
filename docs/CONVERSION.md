# Converting from Szurubooru

## Before Starting

This guide assumes you have cloned the Oxibooru repository but have not yet run it (no `data` or `sql` directories). Simply specify the `MOUNT_DATA` and `MOUNT_SQL` directories in the Oxibooru `.env` file and the conversion script will handle initialization automatically.

The guide doesn't cover migrating the `config.yaml`, but the structure of the Oxibooru equivalent, `config.toml`, is almost identical. Just copy over the settings from the config.yaml manually if you want them.

If you encounter any issues during the conversion process, please open up an issue on [Github](https://github.com/liamw1/oxibooru/issues).

## Known Limitations

Some aspects of a Szurubooru instance can't be converted to an Oxibooru instance. Depending on how you're using your database, these limitations may make total migration difficult or impossible.

1. **Passwords**

    Password hashing is done a bit differently in Oxibooru, so this unfortunately means that passwords can't be migrated over at the moment. Passwords can be reset individually via the admin cli or can be reset via reset requests if SMTP information is provided in the `config.toml`.

2. **Some image formats**

    Currently, Oxibooru doesn't support HEIF or HEIC file formats.

3. **Some post types**

    Oxibooru does not currently support YouTube posts and is unlikely to support them in the future.

## Let's Begin

If you're able to accept these limitations, let's start converting...

The easiest way to convert is using the provided conversion script. Make sure both your Szurubooru and Oxibooru directories have their `.env` files configured with `POSTGRES_USER`, `POSTGRES_DB`, and `MOUNT_DATA` variables.

```sh
./scripts/convert_szuru.sh --szuru-dir /path/to/szurubooru --oxi-dir /path/to/oxibooru
```

By default, the script will copy the Szurubooru data directory. If this is too slow or you do not have enough storage to duplicate this folder, then the `--move-data` flag can be specified to move instead of copy. **But be careful**: the data folder will be modified such that Szurubooru will no longer know how to read it. **Only use this flag if you've made a backup or can accept the risk**.

### Script Options

| Option                        | Description                                                          |
| ----------------------------- | -------------------------------------------------------------------- |
| `--oxi-dir PATH`              | Path to Oxibooru source directory (required)                         |
| `--szuru-dir PATH`            | Path to Szurubooru source directory (required)                       |
| `--szuru-container NAME`      | Szurubooru SQL container name (default: szuru-sql-1)                 |
| `--oxi-sql-container NAME`    | Oxibooru SQL container name (default: oxibooru-sql-1)                |
| `--oxi-server-container NAME` | Oxibooru server container name (default: oxibooru-server-1)          |
| `--move-data`                 | Move the data directory instead of copying (faster, but destructive) |
| `--no-single-transaction`     | Allow partial database conversion on errors                          |

### Reset User Passwords

After running the script, users won't be able to login using their original passwords due to differences in how Oxibooru hashes and salts passwords. To reset a user's password, enter the admin cli:

```sh
docker exec -it oxibooru-server-1 ./server --admin
```

Then run the `reset_password` command and follow the prompts:

```
Please select a task: reset_password
```

Alternatively, if SMTP is configured in `config.toml`, users can use the password reset feature on the login page.

### Notes

- **Case-sensitive tag/pool names**: Pool and tag names are unique and case insensitive in Oxibooru. If your Szurubooru database contains names that only differ by case (e.g., "tag" and "Tag"), they will be renamed to `{name}_name_modified_{tag_id}_{order}`. Search for `*_name_modified_*` in the tag/pool search bar to find affected items.

- **Config migration**: Remember to manually copy your settings from `config.yaml` to `config.toml`.

That's it! Your Oxibooru instance should now be accessible.