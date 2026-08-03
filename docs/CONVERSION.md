# Converting from Szurubooru

## Using the provided docker-compose stack
### Before Starting

This guide assumes you have cloned the Oxibooru repository but have not yet run it (no `data` or `sql` directories). Simply specify the `MOUNT_DATA` and `MOUNT_SQL` directories in the Oxibooru `.env` file and the conversion script will handle initialization automatically.

```
git clone https://github.com/liamw1/oxibooru.git
cd oxibooru
cp example.env .env
```

The guide doesn't cover migrating the `config.yaml`, but the structure of the Oxibooru equivalent, `config.toml`, is almost identical. Just copy over the settings from the config.yaml manually if you want them.

If you encounter any issues during the conversion process, please open up an issue on [Github](https://github.com/liamw1/oxibooru/issues).

### Known Limitations

Some aspects of a Szurubooru instance can't be converted to an Oxibooru instance. Depending on how you're using your database, these limitations may make total migration difficult or impossible.

1. **Passwords**

    Password hashing is done a bit differently in Oxibooru, so this unfortunately means that passwords can't be migrated over at the moment. Passwords can be reset individually via the admin CLI or can be reset via reset requests if SMTP information is provided in the `config.toml`.

2. **Some image formats**

    Currently, Oxibooru doesn't support HEIF or HEIC file formats.

3. **Some post types**

    Oxibooru does not currently support YouTube posts and is unlikely to support them in the future.

### Let's Begin

If you're able to accept these limitations, let's start converting...

The easiest way to convert is using the provided conversion script. Make sure both your Szurubooru and Oxibooru directories have their `.env` files configured with `POSTGRES_USER`, `POSTGRES_DB`, and `MOUNT_DATA` variables.

```sh
./scripts/convert_szuru.sh --szuru-dir /path/to/szurubooru --oxi-dir /path/to/oxibooru
```

By default, the script will copy the Szurubooru data directory. If this is too slow or you do not have enough storage to duplicate this folder, then the `--move-data` flag can be specified to move instead of copy. **But be careful**: the data folder will be modified such that Szurubooru will no longer know how to read it. **Only use this flag if you've made a backup or can accept the risk**.

#### Script Options

| Option                        | Description                                                          |
| ----------------------------- | -------------------------------------------------------------------- |
| `--oxi-dir PATH`              | Path to Oxibooru source directory (required)                         |
| `--szuru-dir PATH`            | Path to Szurubooru source directory (required)                       |
| `--szuru-container NAME`      | Szurubooru SQL container name (default: szuru-sql-1)                 |
| `--oxi-sql-container NAME`    | Oxibooru SQL container name (default: oxibooru-sql-1)                |
| `--oxi-server-container NAME` | Oxibooru server container name (default: oxibooru-server-1)          |
| `--move-data`                 | Move the data directory instead of copying (faster, but destructive) |
| `--no-single-transaction`     | Allow partial database conversion on errors                          |

#### Reset User Passwords

After running the script, users won't be able to login using their original passwords due to differences in how Oxibooru hashes and salts passwords. To reset a user's password, enter the admin CLI:

```sh
docker exec -it oxibooru-server-1 ./server --admin
```

Then run the `reset_passwords` command and follow the prompts:

```
Please select a task: reset_passwords
```

Alternatively, if SMTP is configured in `config.toml`, users can use the password reset feature on the login page.

## Using a remote postgres instance

`convert_szuru.sh` reaches every database through `docker exec <container>`, so it
only works when both databases are services in the compose stack. If your
Postgres runs elsewhere — its own host, a managed instance, anything you reach
over TCP — use `scripts/convert_szuru_generic.sh` instead.

Both databases are addressed by URL and both are required:

```sh
./scripts/convert_szuru_generic.sh \
  --szuru-dir /path/to/szurubooru --oxi-dir /path/to/oxibooru \
  --szuru-db url:postgresql://szuru@db.example:5432/szuruboru \
  --oxi-db   url:postgresql://oxi@db.example:5432/oxibooru
```

If your PostgreSQL runs in a container, publish its port (or use its network
address) and give that as the URL. The Oxibooru **server** is still addressed as
a container or a local binary — it is only the databases that are URL-only.

### Requirements

- read access to the szurubooru database
- plpython3u installed on the database host
- superuser connection to the target database (used to create the extension)
- the user that you will use for oxi needs to have the correct permission to the database you will use for oxibooru

Both databases have to be reachable by URL before the run starts — this script
does not start them. It does start and stop the Oxibooru **server** container,
which must already exist; it never creates one.

#### Stopping Szurubooru

`convert_szuru.sh` ran `docker compose down` on the Szurubooru stack before
dumping, so nothing could write during the conversion. This script never touches
a stack it was not given, so **stopping writers is yours to arrange**. A post
uploaded after the `data` step gets a row in the dump and no file on disk; a post
uploaded after the dump is not carried over at all.

Either stop Szurubooru's `client` and `api` services yourself, or hand the script
the command:

```sh
--quiesce-cmd 'docker compose -f /path/to/szurubooru/docker-compose.yml stop client api'
```

Leave the `sql` service up — the dump reads from it. Preflight warns and asks for
confirmation when no `--quiesce-cmd` was given. `-y`/`--yes` does **not** answer
that one: an unattended run with no `--quiesce-cmd` aborts unless you also pass
`--allow-live-source`, which says in as many words that you accept losing
whatever gets uploaded during the run.

#### Disk space on the database host

**Budget at least 6x the size of the source database, free on the filesystem
holding `PGDATA`.** Measured on a ~500k-post instance:

| | |
| --- | --- |
| **peak while converting** | **~4.9x the source database** |
| converted database, finished | ~1.3x the source database |

```sql
SELECT pg_size_pretty(pg_database_size('szuruboru'));
```

The finished database is only ~1.3x the source, but getting there costs ~5x.
This is not "two copies of the data": `convert_szuru_database.sql` mutates the
restored tables in place before copying them into `oxi.*`, and does so
repeatedly — 11 `UPDATE`s against `public.post` alone. Every `UPDATE` writes a
new row version, and none of it can be reclaimed, because `VACUUM` cannot run
inside the transaction that created the garbage.

#### PL/Python

The package name differs by distro. The major must match the server exactly.

| | PL/Python | SQLAlchemy |
| --- | --- | --- |
| Debian / Ubuntu (PGDG) | `postgresql-plpython3-18`| `python3-sqlalchemy` |
| RHEL / Rocky (PGDG) | `postgresql18-plpython3` | `python3-sqlalchemy` |
| Alpine | `postgresql18-plpython3` | `py3-sqlalchemy` |

SQLAlchemy is not optional decoration: Szurubooru pickles SQLAlchemy-mapped
objects into `snapshot.data`, and unpickling one imports the module that defined
its class. Install it on the **database** host, next to PL/Python — it is
imported by the Postgres backend, not by anything of yours.

#### Superuser

`--superuser-db` should point at the **target database itself**, not at
`postgres`: extensions are per-database, so a superuser connected elsewhere
cannot tell whether `plpython3u` is usable in the one that matters.

The script grants `SUPERUSER` to the Oxibooru role immediately before the
convert step and revokes it afterwards.

### Running it

```sh
./scripts/convert_szuru_generic.sh \
  --oxi-dir      /path/to/oxibooru \
  --szuru-db     url:postgresql://szuru@db.example:5432/szuruboru \
  --oxi-db       url:postgresql://oxi@db.example:5432/oxibooru \
  --superuser-db url:postgresql://postgres@db.example:5432/oxibooru \
  --oxi-server   docker:oxibooru-server-1 \
  --szuru-data   /path/to/szuru/data \
  --oxi-data     /path/to/oxi/data \
  --work-dir     /path/with/room
```

`--work-dir` matters: `backup.sql` is a multi-GB plain-text dump, so keep it off
a small root filesystem.

If you do not want to clone the repository just for one file, replace
`--oxi-dir` with `--convert-sql`:

```sh
curl -O https://raw.githubusercontent.com/liamw1/oxibooru/master/scripts/convert_szuru_database.sql
```

#### Passwords

Nothing is passed on a command line. Each password is exported as `PGPASSWORD`
around the single `psql`/`pg_dump` call that needs it. A password embedded in a
`url:` spec is stripped before the child command is built, but **it is still in
the script's own argv**, visible in `ps` for the entire run. Prefer `~/.pgpass`
(mode `0600`) on the machine running the script:

```
db.example:5432:szuruboru:szuru:<password>
db.example:5432:oxibooru:oxi:<password>
db.example:5432:oxibooru:postgres:<password>
```

`.pgpass` matches on database name, so the superuser needs its own line even
though host and port repeat.

#### Script options

| Option | Description |
| --- | --- |
| `--szuru-db SPEC` | Szurubooru database: `url:postgresql://user@host:port/db` (required) |
| `--oxi-db SPEC` | Oxibooru database, same form (required) |
| `--oxi-server SPEC` | Oxibooru server: `docker:<container>` or `exec:<path-to-binary>` |
| `--superuser-db SPEC` | Superuser connection, used only to grant then revoke `SUPERUSER` around the convert step |
| `--szuru-dir PATH` | Szurubooru directory (reads `.env`, enables the container defaults) |
| `--oxi-dir PATH` | Oxibooru checkout (reads `.env`, supplies `--convert-sql` by default) |
| `--convert-sql PATH` | `convert_szuru_database.sql`, if you would rather not clone. Overrides `--oxi-dir` |
| `--oxi-client SPEC` | `docker:<container>` for the client, started at the end alongside the server |
| `--szuru-data PATH` | Szurubooru data directory (overrides `MOUNT_DATA`) |
| `--oxi-data PATH` | Oxibooru data directory (overrides `MOUNT_DATA`) |
| `--work-dir PATH` | Where to write `backup.sql` (default: `--oxi-dir`, else `$PWD`) |
| `--copy-data` | Copy the data directory (default) |
| `--move-data` | Move it — fast, destructive, Szurubooru can no longer read it |
| `--link-data` | Hard-link it with `cp -al` — fast, no extra disk, same filesystem only |
| `--from STEP` / `--to STEP` | Run a range of steps |
| `--only STEP[,STEP...]` | Run only these steps |
| `--quiesce-cmd CMD` | Shell command run before the data step, to stop writers |
| `--no-single-transaction` | Allow a partial database conversion on error |
| `--keep-dump` | Do not delete `backup.sql` when finished |
| `--dry-run` | Print every command that would run; read-only checks still execute |
| `--force` | Allow resuming into a non-idempotent step |
| `--force-unpickle` | Convert even though the pickled-column probe failed. Separate from `--force` on purpose |
| `--allow-live-source` | Convert without stopping Szurubooru. Required (instead of `-y`) for an unattended run with no `--quiesce-cmd` |
| `--allow-restore-errors` | Continue when psql reports errors restoring the dump. Required (instead of `-y`) for an unattended run |
| `--no-server-env-check` | Skip the server-container/`--oxi-db` consistency check |
| `-y`, `--yes` | Do not prompt for confirmation |

`--oxi-server-container` is accepted as an alias for `--oxi-server docker:…`.
Upstream's `--szuru-container` and `--oxi-sql-container` are **not**: they name
database containers, and databases are addressed by URL here.

The environment variables `MOVE_DATA`, `SINGLE_TRANSACTION`,
`OXI_SERVER_CONTAINER`, `SZURU_DIR` and `OXI_DIR` are honoured; flags override
them. `SZURU_SQL_CONTAINER` and `OXI_SQL_CONTAINER` are ignored, for the same
reason.

#### The server-container check

When `--oxi-server` is a container, preflight compares what that container is
configured to reach (`POSTGRES_DB`, `POSTGRES_HOST`, `POSTGRES_PORT`, and its
`/data` mount) against `--oxi-db` and `--oxi-data`, and refuses to run when they
disagree. This matters most with a remote database: `docker-compose.yml` ships
`POSTGRES_HOST: sql`, so a server container left at that default while `--oxi-db`
points at `db.example` would have the conversion write to one server and
`reset_filenames`/`recompute_checksums`/`recompute_signatures` fix up the other —
finishing "successfully" with every post still carrying its Szurubooru filename,
checksum and signature.

Ports are compared as libpq resolves them, so omitting `:5432` from `--oxi-db`
does not skip the check — an omitted port on either side means 5432, which is
also the server's own `DEFAULT_POSTGRES_PORT`.

A container that sets no `POSTGRES_DB` at all gets a warning rather than silence:
the server has no default for it (it is read with `?` in `server/src/config.rs`),
so either it comes from a `.env` mounted inside the container — which
`docker inspect` cannot see, and this check therefore cannot verify — or the
server will not start.

Paths are compared normalised, so a `MOUNT_DATA` written relative to the compose
file, or with a trailing slash, is not reported as a different directory from the
absolute source `docker inspect` returns.

If the two names genuinely reach the same server by different routes, pass
`--no-server-env-check`.

#### `--link-data`

`cp -al` builds the Oxibooru tree out of hard links, so the data step costs
seconds and no extra disk. This is safe because the only step that touches names
is `reset_filenames`, and renaming one directory entry does not affect the other
link — the Szurubooru tree keeps its original names and stays readable as a
rollback.

Preflight refuses if source and destination are on different filesystems, and
walks the whole source tree for nested mounts (`data/posts/originals` on its own
dataset, say), so a cross-device link failure cannot leave a half-linked tree
behind. That walk needs GNU `find`; without it only the top level is checked and
preflight says so.

### Steps, and resuming after a failure

The run is ten named steps:

```
preflight data dump init restore convert filenames thumbsizes checksums signatures
```

The last four are admin tasks, and the last two are long — roughly 36 and 57
minutes respectively on a ~500k-post instance. They recompute from the files
on disk, so if one dies you can rerun it alone:

```sh
./scripts/convert_szuru_generic.sh ... --only signatures
```

Note that idempotent is not the same as incremental: `recompute_checksums` and
`recompute_signatures` do not skip posts that already have a result, so a rerun
costs the full time again. Steps `data` through `convert` are *not* idempotent
at all; the script refuses to resume into them without `--force`, and says so.

`--force` does exactly that one thing. It does not weaken any other check — in
particular the pickled-column probe that runs before `convert` still stops the
run, since you may well have passed `--force` to re-enter an unrelated step.
Overriding that probe is `--force-unpickle`, separately.

A resumed run re-checks `backup.sql` for pg_dump's completion marker, so a dump
left truncated by a killed `dump` step is refused rather than restored as if it
were whole. It also keeps that dump: only a run that took the dump itself
deletes it at the end, so resuming with `--from checksums` cannot remove the file
it resumed around. (`--keep-dump` still keeps it in every case.)

#### Stopping between `restore` and `convert`

`restore` renames the Oxibooru schema to `oxi` and puts the Szurubooru dump in
`public`. Until `convert` has run, that is what the database holds — so a run
that ends in between (`--to restore`, `--only restore --force`, or a failure in
`convert`) deliberately does **not** start the server again, and says so. A
server started against that database applies diesel's migrations into the
Szurubooru schema, on top of the data you are halfway through migrating.

Finish with `--from convert --force`, then start it.

#### Re-running against a database that was already converted

The conversion ends by dropping the Szurubooru schema and renaming `oxi` back to
`public`, which leaves nothing behind for the "has this already run?" checks to
find — no `alembic_version`, no `oxi` schema. What it cannot hide is the data,
so preflight refuses a target whose `public.post` already has rows. `restore`
wants the Oxibooru schema present and empty.

### Timeouts

| Variable | Default | Meaning |
| --- | --- | --- |
| `MIGRATION_TIMEOUT` | 120 | How long `init` waits for diesel to finish applying migrations |
| `DB_READY_TIMEOUT` | 60 | How long to wait for a database to start accepting connections |

`MIGRATION_TIMEOUT` has a floor of roughly ten seconds, whatever you set. The
step waits for the *count* of applied migrations to stop changing rather than for
diesel's bookkeeping table to appear — the table is created before the first
migration is applied, so waiting for it used to stop the server mid-migration —
and confirming it has settled costs two poll intervals. If the clock runs out
while migrations are still being applied, the script says exactly that rather
than blaming the server for not starting.

## After the conversion

These apply however you converted.

- **Case-sensitive tag/pool names**: Pool and tag names are unique and case insensitive in Oxibooru. If your Szurubooru database contains names that only differ by case (e.g., "tag" and "Tag"), they will be renamed to `{name}_name_modified_{tag_id}_{order}`. Search for `*_name_modified_*` in the tag/pool search bar to find affected items.

- **Config migration**: Remember to manually copy your settings from `config.yaml` to `config.toml`.

- **API tokens do not survive**: `user_token` is not carried over by
  `convert_szuru_database.sql`. Anything automated against the Szurubooru API —
  an importer, a bot, a backup script — needs a new token issued after the
  conversion, in addition to `reset_passwords`.

- **The runtime image has no shell**: `server/Dockerfile` ends `FROM scratch`, so
  `docker exec -it oxibooru-server-1 /bin/bash` fails. Use
  `docker exec -it oxibooru-server-1 ./server --admin` directly. This also means
  a running admin task cannot be signalled from inside the container, and killing
  a `docker exec` client does not stop the process it started — `docker stop`
  does.

That's it! Your Oxibooru instance should now be accessible.