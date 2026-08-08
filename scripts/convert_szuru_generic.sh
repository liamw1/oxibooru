#!/usr/bin/env bash
# =============================================================================
# Szurubooru to Oxibooru conversion
# =============================================================================
# Converts a Szurubooru database and data directory to Oxibooru format.
# Databases are addressed by URL; the Oxibooru server is a container or a local
# binary. See docs/CONVERSION.md.
#
# Known limitations, inherited from upstream and not fixable here:
# - Passwords cannot be migrated (must be reset manually)
# - HEIF/HEIC file formats are not supported
# - YouTube posts are not supported
# =============================================================================

# ${ROLE}_DB_* are read by indirect expansion, which shellcheck cannot follow.
# shellcheck disable=SC2034
# -E so the ERR trap reaches functions and command substitutions. Set only when
# executed: sourcing this file must not leave errexit on in the caller's shell.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    set -Eeuo pipefail
fi

# "${STEPS[-1]}" needs 4.3; "${empty_array[@]}" under set -u needs 4.4.
if (( BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4) )); then
    echo "This script needs bash >= 4.4 (found ${BASH_VERSION})." >&2
    # `return` when sourced; `exit` here would close the caller's shell, and the
    # one shell old enough to fail this test is the one still using bash 3.2.
    return 1 2>/dev/null || exit 1
fi

# -----------------------------------------------------------------------------
# Output helpers
# -----------------------------------------------------------------------------

if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
    BLUE='\033[0;34m'; NC='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; NC=''
fi

print_header()  { echo -e "\n${BLUE}=============================================================================${NC}"
                  echo -e "${BLUE}$1${NC}"
                  echo -e "${BLUE}=============================================================================${NC}\n"; }
print_step()    { echo -e "${GREEN}[${1}]${NC} $2"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1" >&2; }
print_error()   { echo -e "${RED}[ERROR]${NC} $1" >&2; }
print_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }

die() { print_error "$1"; exit 1; }

confirm() {
    if [[ "$ASSUME_YES" == true ]]; then
        return 0
    fi
    local reply
    read -r -p "$1 (y/N): " reply
    [[ "$reply" =~ ^[Yy]$ ]]
}

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------

SZURU_DIR="${SZURU_DIR:-}"
OXI_DIR="${OXI_DIR:-}"
WORK_DIR=""

SRC_DB_SPEC=""
TGT_DB_SPEC=""
SU_DB_SPEC=""
OXI_SERVER_SPEC=""

# Captured before the internal variable of the same name is initialised below.
ENV_OXI_SERVER_CONTAINER="${OXI_SERVER_CONTAINER:-}"

# Parsed connection state, indirectly addressed as ${ROLE}_DB_* where ROLE is
# SRC (szurubooru), TGT (oxibooru) or SU (superuser, for privilege grants).
SRC_DB_URL=""; SRC_DB_USER=""; SRC_DB_NAME=""; SRC_DB_PASSWORD=""
TGT_DB_URL=""; TGT_DB_USER=""; TGT_DB_NAME=""; TGT_DB_PASSWORD=""
SU_DB_URL="";  SU_DB_USER="";  SU_DB_NAME="";  SU_DB_PASSWORD=""
# Host and port are kept separately from the URL: the server-container check and
# the plpython3u probe both need to know whether two specs address the same
# server, which a rebuilt URL string cannot answer.
SRC_DB_HOST=""; SRC_DB_PORT=""
TGT_DB_HOST=""; TGT_DB_PORT=""
SU_DB_HOST="";  SU_DB_PORT=""

OXI_SERVER_MODE=""; OXI_SERVER_CONTAINER=""; OXI_SERVER_BIN=""
OXI_CLIENT_CONTAINER=""
CONVERT_SQL_OVERRIDE=""
MIGRATION_TIMEOUT="${MIGRATION_TIMEOUT:-120}"

SZURU_MOUNT_DATA=""
OXI_MOUNT_DATA=""

# Stamped on every connection (libpq reads PGAPPNAME) so the interrupt handler can
# end this script's sessions and nobody else's. Exported by main(), not on source.
APP_NAME="convert_szuru_generic"

# The operator's own PGPASSWORD, captured once, before anything overwrites it.
# with_pgpassword falls back to THIS rather than to whatever the previous call
# happened to leave exported.
HAD_ORIG_PGPASSWORD=false
ORIG_PGPASSWORD=""
if [[ -n "${PGPASSWORD+x}" ]]; then
    HAD_ORIG_PGPASSWORD=true
    ORIG_PGPASSWORD="$PGPASSWORD"
fi

# Honour upstream's environment variables; command-line flags are parsed after.
DATA_MODE="copy"           # copy | move | link
if [[ "${MOVE_DATA:-false}" == true ]]; then
    DATA_MODE="move"
fi
SINGLE_TRANSACTION="${SINGLE_TRANSACTION:-true}"
DRY_RUN=false
ASSUME_YES=false
FORCE=false
# Deliberately NOT --force. --force is what the resume hints tell an operator to
# pass after an unrelated step died, and overloading it would silently disarm the
# one guard that stands between bad pickled data and a multi-hour rollback.
FORCE_UNPICKLE=false
KEEP_DUMP=false
QUIESCE_CMD=""
# Explicit "yes, Szurubooru may keep running". Deliberately not covered by -y:
# see the quiesce block in step_preflight.
ALLOW_LIVE_SOURCE=false
# Explicit "yes, restore the dump even though psql reported errors". Also not
# covered by -y, for the same reason: see step_restore.
ALLOW_RESTORE_ERRORS=false
# Escape hatch for the server-container/target-database consistency check, for
# the setups where the two legitimately disagree by name (a docker network alias
# reaching the same server under a different hostname, say).
SKIP_SERVER_ENV_CHECK=false

FROM_STEP=""
TO_STEP=""
ONLY_STEPS=()
ONLY_GIVEN=""
only_valid=false

STEPS=(preflight data dump init restore convert filenames thumbsizes checksums signatures)
# Steps that mutate what an earlier step produced. dump is absent on purpose: it
# only overwrites backup.sql, so requiring --force would train reflexive --force.
NON_IDEMPOTENT_STEPS=(data init restore convert)

CURRENT_STEP="startup"
SUPERUSER_GRANTED=false
QUIESCED=false
DUMP_FILE=""
# True while the server container is down at this script's hands, so every exit
# path can restart it or say it is down. Cleared by start_oxi_stack.
SERVER_STOPPED_BY_SCRIPT=false
# Whether it was already running when we arrived. Starting one that was down
# before is not 'putting it back' -- `--to init` stops a server it started.
SERVER_WAS_RUNNING=false
# Set once on_error or on_interrupt has printed the recovery advice, so the EXIT
# trap does not print it a second time.
ERROR_REPORTED=false
# Set once the schema rename commits: recovery advice differs on either side.
RESTORE_RENAMED=false

# How long to wait for a database to accept connections after its container is
# started. The postgres image runs its init scripts on first boot, which on a
# fresh stack is the difference between "not ready" and "not there".
DB_READY_TIMEOUT="${DB_READY_TIMEOUT:-60}"

# -----------------------------------------------------------------------------
# Signal handling and cleanup
# -----------------------------------------------------------------------------

revoke_superuser_if_granted() {
    if [[ "$SUPERUSER_GRANTED" == true ]]; then
        print_info "Revoking temporary superuser from '$TGT_DB_ROLE'..."
        # Cleared only on success, so a failed revoke leaves the retry traps armed.
        if db_exec SU -c "ALTER ROLE \"$TGT_DB_ROLE\" NOSUPERUSER;" >/dev/null 2>&1; then
            SUPERUSER_GRANTED=false
        else
            print_warning "Failed to revoke superuser from '$TGT_DB_ROLE'. Will retry on exit; if that also fails, revoke it by hand:"
            print_warning "    ALTER ROLE \"$TGT_DB_ROLE\" NOSUPERUSER;"
        fi
    fi
}

# Last-chance revoke, on the way out. By this point every earlier attempt has
# failed, so say so where it cannot be missed rather than exiting quietly.
revoke_superuser_final() {
    revoke_superuser_if_granted
    if [[ "$SUPERUSER_GRANTED" == true ]]; then
        print_error "SUPERUSER IS STILL GRANTED to '$TGT_DB_ROLE' and could not be revoked."
        print_error "Revoke it manually: ALTER ROLE \"$TGT_DB_ROLE\" NOSUPERUSER;"
    fi
}

on_exit() {
    # First statement in the function: everything below clobbers $?.
    local status=$?
    revoke_superuser_final
    # A server container this script stopped is only restarted on the success
    # path. If we are leaving without having done that, say so -- otherwise the
    # instance stays down and nothing on screen mentions it.
    if [[ "$SERVER_STOPPED_BY_SCRIPT" == true ]]; then
        print_warning "The Oxibooru server container is stopped (this script stopped it). Start it when you are ready:"
        print_warning "    docker start $OXI_SERVER_CONTAINER"
    fi
    # bash runs the ERR trap for a failed command, never for `exit`, so every die()
    # reaches here instead. ERROR_REPORTED stops the hint printing twice.
    if (( status != 0 )) && [[ "$ERROR_REPORTED" != true ]] && hint_applicable; then
        resume_hint "$CURRENT_STEP"
    fi
}

# Whether a resume hint makes sense for the step we are leaving from. preflight
# and startup precede every state change, and `cleanup` is not a member of STEPS
# at all -- `--from cleanup` is not a command anyone can run.
hint_applicable() {
    # A --dry-run mutates nothing: run() prints instead of executing, and run_ro
    # is read-only. Recovery advice would prescribe dropping a database to undo
    # changes that were never made.
    if [[ "$DRY_RUN" == true ]]; then
        return 1
    fi
    case "$CURRENT_STEP" in
        startup|preflight|cleanup) return 1 ;;
        *) return 0 ;;
    esac
}

resume_hint() {
    local step="$1"
    # Two steps leave state that --force alone cannot get past, so name the cleanup.
    case "$step" in
        restore)
            if [[ "$RESTORE_RENAMED" == true ]]; then
                print_info "restore renames public -> oxi as its first statement, and that has committed."
                print_info "Recover: drop and recreate the Oxibooru database, then re-run the diesel"
                print_info "migrations and continue with: --from init --force"
                print_info "Resuming with --from restore alone will be refused: preflight sees the"
                print_info "leftover 'oxi' schema."
            else
                print_info "The schema rename did not commit, so the database is unchanged."
                print_info "The usual cause is that the Oxibooru role does not own the 'public'"
                print_info "schema -- on PostgreSQL 15+ it is owned by pg_database_owner, so a"
                print_info "role that merely has rights on the database cannot rename it:"
                print_info "    ALTER SCHEMA public OWNER TO \"${TGT_DB_ROLE:-<oxi role>}\";   -- as a superuser"
                print_info "Fix the cause and resume with: --from restore --force"
            fi
            ;;
        data)
            case "$DATA_MODE" in
                move)
                    print_info "This was a MOVE. Whatever mv already transferred exists ONLY under"
                    print_info "${OXI_MOUNT_DATA:-<oxi data dir>} -- it is the sole copy of those files."
                    print_info "Do NOT delete it. Merge it back into ${SZURU_MOUNT_DATA:-<szuru data dir>}"
                    print_info "(cp -al / rsync then remove), or finish the move by hand."
                    print_info "Once ${OXI_MOUNT_DATA:-<oxi data dir>} no longer exists: --only data --force"
                    ;;
                link)
                    print_info "A partial data step leaves the destination behind, which the next run refuses."
                    print_info "Recover: rm -rf ${OXI_MOUNT_DATA:-<oxi data dir>}   # hard links, so this frees nothing yet"
                    print_info "Then: --only data --force"
                    ;;
                *)
                    print_info "A partial data step leaves the destination behind, which the next run refuses."
                    print_info "Recover: rm -rf ${OXI_MOUNT_DATA:-<oxi data dir>}   # a partial copy; the source is untouched"
                    print_info "Then: --only data --force"
                    ;;
            esac
            ;;
        convert)
            print_info "Recover: drop and recreate the Oxibooru database, then: --from init --force"
            print_info "The conversion is not restartable in place -- it writes into oxi.* from a"
            print_info "public schema that it consumes as it goes."
            ;;
        *)
            if is_non_idempotent "$step"; then
                print_info "Resume with: --from $step --force"
            else
                print_info "Resume with: --from $step"
            fi
            ;;
    esac
}

on_error() {
    local status="$1"
    trap - ERR
    ERROR_REPORTED=true
    print_error "Failed during step '$CURRENT_STEP' (exit $status)"
    if hint_applicable; then
        resume_hint "$CURRENT_STEP"
    fi
}

# The background jobs of THIS shell, which is what `jobs -p` reports and nothing
# else -- no tracking, and no window between starting a job and recording it.
# The old `pkill -P $$` killed every child of the running shell instead, which is
# the operator's own when main() is called from a shell that sourced this file.
kill_background() {
    local p
    for p in $(jobs -p 2>/dev/null); do
        kill "$p" 2>/dev/null || true
    done
}

on_interrupt() {
    trap - SIGINT SIGTERM
    echo ""
    print_warning "Caught interrupt signal, cleaning up..."
    kill_background
    if [[ "$OXI_SERVER_MODE" == "docker" && -n "$OXI_SERVER_CONTAINER" ]]; then
        # No pkill inside the server image (FROM scratch), and killing the exec
        # client does not stop the task. Stopping the container does.
        if [[ "$(docker inspect -f '{{.State.Running}}' "$OXI_SERVER_CONTAINER" 2>/dev/null)" == "true" ]]; then
            print_info "Stopping $OXI_SERVER_CONTAINER to end any running admin task..."
            docker stop "$OXI_SERVER_CONTAINER" >/dev/null 2>&1 || true
            SERVER_STOPPED_BY_SCRIPT=true
        fi
    fi

    # The conversion runs inside PostgreSQL, where killing psql leaves the backend
    # working. End our own sessions -- application_name makes that precise.
    if [[ -n "$TGT_DB_URL" ]]; then
        print_info "Ending this script's database sessions on $(db_label TGT)..."
        db_query TGT "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
                       WHERE application_name = '$APP_NAME' AND pid <> pg_backend_pid()" \
            >/dev/null 2>&1 || true
    fi

    revoke_superuser_if_granted
    ERROR_REPORTED=true
    if hint_applicable; then
        resume_hint "$CURRENT_STEP"
    fi
    print_info "Cleanup complete. Exiting."
    exit 130
}

# Installed by main(), not at load time: sourcing this file for its functions
# (a test harness does) must not leave an EXIT trap behind in the caller.
install_traps() {
    trap 'on_error $?' ERR
    trap on_exit EXIT
    trap on_interrupt SIGINT SIGTERM
}

# -----------------------------------------------------------------------------
# Command execution
# -----------------------------------------------------------------------------

# Replace any known password with *** before a command is echoed.
redact() {
    local s="$1" pw
    for pw in "$SRC_DB_PASSWORD" "$TGT_DB_PASSWORD" "$SU_DB_PASSWORD"; do
        if [[ -n "$pw" ]]; then
            s="${s//"$pw"/***}"
        fi
    done
    printf '%s' "$s"
}

fmt_cmd() {
    local out='' arg
    for arg in "$@"; do
        out+="$(printf '%q ' "$(redact "$arg")")"
    done
    printf '%s' "${out% }"
}

# Run a mutating command. Honours --dry-run.
run() {
    if [[ "$DRY_RUN" == true ]]; then
        printf '%b[dry-run]%b %s\n' "$YELLOW" "$NC" "$(fmt_cmd "$@")" >&2
        return 0
    fi
    "$@"
}

# Run a read-only command. Always executes, even under --dry-run, so that
# preflight still validates a real environment.
run_ro() {
    if [[ "$DRY_RUN" == true ]]; then
        printf '%b[dry-run:ro]%b %s\n' "$YELLOW" "$NC" "$(fmt_cmd "$@")" >&2
    fi
    "$@"
}

# -----------------------------------------------------------------------------
# Connection specs
# -----------------------------------------------------------------------------

# Percent-decoding, but only for well-formed %XX. A bare % or a backslash must
# survive untouched: they are legal in a password, and printf '%b' would eat both.
urldecode() {
    local s="$1" out='' c ch
    while [[ -n "$s" ]]; do
        c="${s:0:1}"
        if [[ "$c" == "%" && "${s:1:2}" =~ ^[0-9A-Fa-f]{2}$ ]]; then
            # printf -v: the pipeline warned on every escape and ate a decoded %0A.
            printf -v ch '%b' "\\x${s:1:2}"
            out+="$ch"
            s="${s:3}"
        else
            out+="$c"
            s="${s:1}"
        fi
    done
    printf '%s' "$out"
}

# Splits a PostgreSQL URL into user / password / host / port / dbname and
# rebuilds a URL without the password, so the password never reaches argv.
parse_db_url() {
    local role="$1" url="$2"
    local rest authority dbpart userinfo hostport user user_raw pass host port db query

    # These fire before the password is captured, so redact() would have nothing to
    # blank: name the flag rather than echoing a URL that still holds it.
    local flag
    case "$role" in
        SRC) flag="--szuru-db" ;;
        TGT) flag="--oxi-db" ;;
        SU)  flag="--superuser-db" ;;
        *)   flag="$role" ;;
    esac

    if [[ ! "$url" =~ ^(postgresql|postgres)://(.+)$ ]]; then
        die "$flag is not a PostgreSQL URL (not shown: it may contain the password). Expected postgresql://user@host:port/dbname"
    fi
    rest="${BASH_REMATCH[2]}"

    if [[ "$rest" != */* ]]; then
        die "$flag is missing the database name (not shown: it may contain the password). Expected postgresql://user@host:port/dbname"
    fi
    authority="${rest%%/*}"
    dbpart="${rest#*/}"

    if [[ "$authority" == *@* ]]; then
        userinfo="${authority%@*}"
        hostport="${authority##*@}"
    else
        userinfo=""
        hostport="$authority"
    fi

    # Both forms are needed: the decoded one authenticates, the raw one goes back
    # into the rebuilt URL. Re-encoding is not optional -- Azure's `oxi%40pgsrv`
    # decoded into a URL moves the host boundary, and `us%3Aer` would come back as
    # user `us` with password `er`.
    if [[ "$userinfo" == *:* ]]; then
        user_raw="${userinfo%%:*}"
        user="$(urldecode "$user_raw")"
        pass="$(urldecode "${userinfo#*:}")"
    else
        user_raw="$userinfo"
        user="$(urldecode "$user_raw")"
        pass=""
    fi

    # A bracketed IPv6 literal keeps its brackets; only a port after the closing
    # bracket is a port separator.
    if [[ "$hostport" == \[*\]* ]]; then
        host="${hostport%%\]*}]"
        port="${hostport#"$host"}"
        port="${port#:}"
    else
        host="${hostport%%:*}"
        port=""
        if [[ "$hostport" == *:* ]]; then
            port="${hostport##*:}"
        fi
    fi

    db="${dbpart%%\?*}"
    if [[ -z "$db" ]]; then
        die "$flag is missing the database name (not shown: it may contain the password). Expected postgresql://user@host:port/dbname"
    fi
    # Preserve connection parameters. Dropping sslmode=require on a database
    # reached across hosts would be a silent security downgrade.
    query=""
    if [[ "$dbpart" == *\?* ]]; then
        query="?${dbpart#*\?}"
    fi

    # An unencoded '/' in the password eats the authority split: host and port come
    # out as fragments of it and redact() never captured it, so keep the URL out of
    # the message.
    if [[ -n "$port" && ! "$port" =~ ^[0-9]+$ ]] || [[ "$db" == *[/@]* ]]; then
        die "Could not parse the $flag URL (not shown: it may still contain the password). If the password contains '/' or '%', percent-encode them as %2F and %25. '!', ':' and '@' need no encoding."
    fi

    local sanitized="postgresql://"
    if [[ -n "$user_raw" ]]; then
        sanitized+="${user_raw}@"
    fi
    sanitized+="$host"
    if [[ -n "$port" ]]; then
        sanitized+=":$port"
    fi
    sanitized+="/${db}${query}"

    printf -v "${role}_DB_URL" '%s' "$sanitized"
    printf -v "${role}_DB_USER" '%s' "$user"
    printf -v "${role}_DB_HOST" '%s' "$host"
    printf -v "${role}_DB_PORT" '%s' "$port"
    printf -v "${role}_DB_NAME" '%s' "$db"
    if [[ -n "$pass" ]]; then
        printf -v "${role}_DB_PASSWORD" '%s' "$pass"
    fi
}

parse_db_spec() {
    local role="$1" spec="$2"
    case "$spec" in
        url:*)                    parse_db_url "$role" "${spec#url:}" ;;
        postgresql://*|postgres://*) parse_db_url "$role" "$spec" ;;
        docker:*)
            die "Databases are addressed by URL, not by container: '$spec'. Publish the port (or use the container's network address) and pass url:postgresql://user@host:port/dbname"
            ;;
        *)
            die "Unrecognised connection spec: '$spec'. Use url:postgresql://user@host:port/dbname"
            ;;
    esac
}

parse_server_spec() {
    local spec="$1"
    case "$spec" in
        docker:*) OXI_SERVER_MODE="docker"; OXI_SERVER_CONTAINER="${spec#docker:}" ;;
        exec:*)   OXI_SERVER_MODE="exec";   OXI_SERVER_BIN="${spec#exec:}" ;;
        *)        die "Unrecognised server spec: '$spec'. Use docker:<container> or exec:<path-to-server-binary>" ;;
    esac
}

# -----------------------------------------------------------------------------
# Database shims
#
# Every database call goes through these; psql and pg_dump run locally, against
# a URL.
# -----------------------------------------------------------------------------

build_db_cmd() {
    local role="$1" prog="$2"; shift 2
    local url_var="${role}_DB_URL"
    [[ -n "${!url_var}" ]] || die "Internal error: connection role $role is not configured"

    # -w never prompts (a prompt goes to /dev/tty and hangs unseen); -X skips
    # ~/.psqlrc, where a stray `\set AUTOCOMMIT off` would change the conversion.
    DB_CMD=("$prog" -w)
    [[ "$prog" == psql ]] && DB_CMD+=(-X)
    DB_CMD+=(-d "${!url_var}" "$@")
}

# PGPASSWORD is saved and restored rather than blindly unset: the operator may
# have exported one for their own use.
with_pgpassword() {
    local role="$1"; shift
    local pw_var="${role}_DB_PASSWORD"
    local status=0
    local had_pgpassword=false saved_pgpassword=""

    if [[ -n "${PGPASSWORD+x}" ]]; then
        had_pgpassword=true
        saved_pgpassword="$PGPASSWORD"
    fi

    # Deterministic, never additive: this role's password, else the operator's own,
    # else none. Leaving the previous call's export meant a trap firing mid-command
    # ran its psql as another role with the wrong password.
    if [[ -n "${!pw_var}" ]]; then
        export PGPASSWORD="${!pw_var}"
    elif [[ "$HAD_ORIG_PGPASSWORD" == true ]]; then
        export PGPASSWORD="$ORIG_PGPASSWORD"
    else
        unset PGPASSWORD
    fi
    "$@" || status=$?
    if [[ "$had_pgpassword" == true ]]; then
        export PGPASSWORD="$saved_pgpassword"
    else
        unset PGPASSWORD
    fi
    return "$status"
}

# Mutating psql/pg_dump invocation (respects --dry-run).
db_exec() {
    local role="$1"; shift
    build_db_cmd "$role" psql -v ON_ERROR_STOP=1 "$@"
    with_pgpassword "$role" run "${DB_CMD[@]}"
}

# Mutating psql invocation without ON_ERROR_STOP, matching upstream's tolerance
# for benign errors while restoring a dump.
db_exec_lenient() {
    local role="$1"; shift
    build_db_cmd "$role" psql "$@"
    with_pgpassword "$role" run "${DB_CMD[@]}"
}

# Execute a SQL file. $3 selects ON_ERROR_STOP: `strict` aborts at the first
# error, `lenient` matches upstream's tolerance while restoring a dump.
db_exec_file() {
    local role="$1" file="$2" strictness="$3"; shift 3
    if [[ "$strictness" == strict ]]; then
        db_exec "$role" "$@" -f "$file"
    else
        db_exec_lenient "$role" "$@" -f "$file"
    fi
}

# Read-only scalar query. ONE statement: psql before v15 prints only the last
# command's result from a multi-statement -c, so anything longer goes through
# db_query_script.
db_query() {
    local role="$1" sql="$2"
    build_db_cmd "$role" psql -tAq -v ON_ERROR_STOP=1 -c "$sql"
    with_pgpassword "$role" run_ro "${DB_CMD[@]}"
}

# Read-only multi-statement query, fed to psql as a script on stdin.
#
# `-f -` rather than -c: psql prints every command's result when reading a
# script, on every version. With -c on psql 14 and older, a probe that ends
# `SELECT ...; ROLLBACK;` returns the ROLLBACK's (suppressed, under -q) result
# and discards the SELECT -- an empty answer, which every caller here reads as
# "nothing wrong". Silently failing open is not something a guard may do.
#
db_query_script() {
    local role="$1" sql="$2"
    build_db_cmd "$role" psql -tAq -v ON_ERROR_STOP=1 -f -
    with_pgpassword "$role" run_ro "${DB_CMD[@]}" <<< "$sql"
}

db_dump() {
    local role="$1"; shift
    build_db_cmd "$role" pg_dump --no-owner --no-privileges "$@"
    with_pgpassword "$role" run "${DB_CMD[@]}"
}

db_reachable() {
    db_query "$1" "SELECT 1" >/dev/null 2>&1
}

# Under --dry-run the environment may not exist at all (reviewing a plan from a
# workstation). Query-driven logic is skipped rather than fatal in that case.
db_usable() {
    if [[ "$DRY_RUN" == true ]] && ! db_reachable "$1"; then
        return 1
    fi
    return 0
}

db_major_version() {
    local role="$1" num
    num="$(db_query "$role" "SHOW server_version_num")"
    # An empty result would arithmetic-evaluate to 0, which would silently pass
    # the pg_dump version comparison and print "postgresql-plpython3-0".
    if [[ ! "$num" =~ ^[0-9]+$ ]]; then
        die "Could not read server_version_num from $(db_label "$role")"
    fi
    printf '%s' $(( num / 10000 ))
}

# True when two roles address the same server. Database-name equality is not
# enough: two clusters can both have an "oxibooru".
same_server() {
    local a_h="${1}_DB_HOST" b_h="${2}_DB_HOST"
    local a_p="${1}_DB_PORT" b_p="${2}_DB_PORT"
    # An omitted port means 5432 to libpq, so compare the effective one.
    [[ "${!a_h}" == "${!b_h}" && "${!a_p:-5432}" == "${!b_p:-5432}" ]]
}

db_label() {
    local url_var="${1}_DB_URL"
    printf '%s' "${!url_var:-(unconfigured)}"
}

# -----------------------------------------------------------------------------
# Oxibooru admin CLI shim
#
# Task names come from oxibooru's AdminTask enum, which serialises snake_case.
# Post-scoped tasks prompt for a post filter before running, so they need one
# extra newline on stdin.
#
# Caveat, inherited from upstream and not fixable from here: `server --admin`
# reports task errors on its own output and then returns to its input loop, so
# the trailing `exit` still leaves a zero exit status. A task that failed on
# every post looks identical to one that succeeded. Read the task's output.
# -----------------------------------------------------------------------------

oxi_admin() {
    local task="$1" post_scoped="${2:-false}"
    local extra=""
    if [[ "$post_scoped" == true ]]; then
        extra=$'\n'
    fi

    if [[ "$DRY_RUN" == true ]]; then
        case "$OXI_SERVER_MODE" in
            docker) run docker exec -i "$OXI_SERVER_CONTAINER" ./server --admin "<<< $task" ;;
            exec)   run "$OXI_SERVER_BIN" --admin "<<< $task" ;;
            *)      die "Oxibooru server is not configured (use --oxi-server)" ;;
        esac
        return 0
    fi

    case "$OXI_SERVER_MODE" in
        docker) printf '%s\n%sexit\n' "$task" "$extra" | docker exec -i "$OXI_SERVER_CONTAINER" ./server --admin ;;
        exec)   printf '%s\n%sexit\n' "$task" "$extra" | "$OXI_SERVER_BIN" --admin ;;
        *)      die "Oxibooru server is not configured (use --oxi-server)" ;;
    esac
}

# -----------------------------------------------------------------------------
# .env parsing (upstream compatibility)
# -----------------------------------------------------------------------------

# The first assignment of a variable, verbatim apart from a trailing CR.
read_env_raw() {
    local env_file="$1" var_name="$2" value
    if [[ ! -f "$env_file" ]]; then
        return 1
    fi
    value=$(grep -E "^${var_name}=" "$env_file" 2>/dev/null | head -n1 | cut -d'=' -f2- || true)
    printf '%s' "${value%$'\r'}"
}

# A plain .env value: MOUNT_DATA, POSTGRES_USER, POSTGRES_DB.
#
# Upstream ended this with `sed 's/#.*//' | xargs`, which mangles the two values
# this script then hands to `mv` and `cp -al`: MOUNT_DATA=/tank/booru#1/data
# became /tank/booru, and `/tank/my  data` had its interior whitespace collapsed.
# With --move-data that is an mv against a path the operator never named.
#
# Comments are still stripped, by docker compose's own rule: an inline comment
# must be preceded by whitespace, and a quoted value is taken literally. A '#'
# inside a path is therefore part of the path, which is what compose itself does
# with the same file.
read_env_var() {
    local value
    value="$(read_env_raw "$1" "$2")" || return 1

    if [[ "$value" == \"*\" || "$value" == \'*\' ]]; then
        printf '%s' "${value:1:${#value}-2}"
        return 0
    fi

    value="${value%%[[:space:]]#*}"
    # Trim, without xargs: it collapses interior whitespace and fails outright on
    # an unbalanced quote.
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    printf '%s' "$value"
}

# Passwords need gentler treatment still. Upstream never read POSTGRES_PASSWORD,
# so its comment stripping was harmless there; here it would turn `hunter2#4`
# into `hunter2`. Strip one layer of matched quotes, and nothing else -- a
# password may legitimately begin, end or consist of whitespace and '#'.
read_env_secret() {
    local value
    value="$(read_env_raw "$1" "$2")" || return 1
    if [[ "$value" == \"*\" || "$value" == \'*\' ]]; then
        value="${value:1:${#value}-2}"
    fi
    printf '%s' "$value"
}

# An absolute path with no trailing slash and no '..' left in it.
#
# MOUNT_DATA in a .env is relative to the directory of the compose file that
# reads it, which is how docker resolves it; `docker inspect` then reports the
# mount source absolute. Comparing the raw .env string against that answer
# called two spellings of one directory a mismatch -- and `data/` vs `/srv/oxi/
# data` was enough to do it. The same normalisation is what `mv` and `cp -al`
# should be given, so it happens once, where the value is read.
abs_path() {
    local p="$1" base="${2:-$PWD}"
    [[ -z "$p" ]] && return 0
    [[ "$p" != /* ]] && p="${base:-$PWD}/$p"
    if realpath -m / >/dev/null 2>&1; then
        realpath -m "$p"
    else
        # No GNU realpath (macOS): trailing slashes are the common case anyway.
        while [[ "$p" == */ && "$p" != "/" ]]; do p="${p%/}"; done
        printf '%s' "$p"
    fi
}

# -----------------------------------------------------------------------------
# Step selection
# -----------------------------------------------------------------------------

step_index() {
    local name="$1" i
    for i in "${!STEPS[@]}"; do
        if [[ "${STEPS[$i]}" == "$name" ]]; then
            printf '%s' "$i"
            return 0
        fi
    done
    die "Unknown step: '$name'. Valid steps: ${STEPS[*]}"
}

is_non_idempotent() {
    local name="$1" s
    for s in "${NON_IDEMPOTENT_STEPS[@]}"; do
        if [[ "$s" == "$name" ]]; then
            return 0
        fi
    done
    return 1
}

should_run() {
    local name="$1" s
    # Preflight is not selectable: its guards are what make resuming safe, so
    # skipping it is never what the caller meant.
    if [[ "$name" == "preflight" ]]; then
        return 0
    fi
    if [[ ${#ONLY_STEPS[@]} -gt 0 ]]; then
        for s in "${ONLY_STEPS[@]}"; do
            if [[ "$s" == "$name" ]]; then
                return 0
            fi
        done
        return 1
    fi
    local idx from_idx to_idx
    idx="$(step_index "$name")"
    from_idx="$(step_index "$FROM_STEP")"
    to_idx="$(step_index "$TO_STEP")"
    (( idx >= from_idx && idx <= to_idx ))
}

run_step() {
    local name="$1"; shift
    if ! should_run "$name"; then
        return 0
    fi
    CURRENT_STEP="$name"
    print_header "Step: $name"
    "$@"
}

# -----------------------------------------------------------------------------
# Usage
# -----------------------------------------------------------------------------

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Converts a Szurubooru database and data directory to Oxibooru format. Both
databases are addressed by URL, wherever they run.

Connection options (both databases are required):
    --szuru-db SPEC       Szurubooru database   url:postgresql://user@host:port/db
    --oxi-db SPEC         Oxibooru database     url:postgresql://user@host:port/db
    --oxi-server SPEC     Oxibooru server       docker:<container> | exec:<path-to-server-binary>
    --superuser-db SPEC   A superuser connection used only to grant, then revoke,
                          SUPERUSER on the oxibooru role around the convert step.
                          Needed because CREATE EXTENSION plpython3u is untrusted DDL.

Directory options:
    --szuru-dir PATH      Szurubooru source directory (reads MOUNT_DATA and the password from .env)
    --oxi-dir PATH        Oxibooru checkout (reads .env, and supplies --convert-sql by default)
    --convert-sql PATH    scripts/convert_szuru_database.sql, if you would rather not clone.
                          Overrides --oxi-dir. curl it straight from the oxibooru repo.
    --oxi-client SPEC     docker:<container> for the oxibooru client, started at the end
                          alongside the server. Omit and bringing the stack up is yours.
    --szuru-data PATH     Szurubooru data directory (overrides MOUNT_DATA from .env)
    --oxi-data PATH       Oxibooru data directory (overrides MOUNT_DATA from .env)
    --work-dir PATH       Where to write backup.sql (default: --oxi-dir, else \$PWD)

Data directory mode (default: copy):
    --copy-data           Copy the data directory
    --move-data           Move it (fast, destructive: Szurubooru can no longer read it)
    --link-data           Hard-link it with 'cp -al' (fast, no extra disk, same filesystem
                          only). Safe here because reset_filenames renames directory
                          entries rather than rewriting file contents, so the Szurubooru
                          tree keeps working as a rollback. Do NOT run regenerate_thumbnails
                          while the trees are linked.

Step selection (steps: ${STEPS[*]}):
    --from STEP           Start at STEP (default: first)
    --to STEP             Stop after STEP (default: last)
    --only STEP[,STEP...] Run only these steps (mutually exclusive with --from/--to)

Environment variables (upstream compatibility): MOVE_DATA, SINGLE_TRANSACTION,
OXI_SERVER_CONTAINER, SZURU_DIR, OXI_DIR.
Command-line flags override them.

Other options:
    --quiesce-cmd CMD     Shell command run before the dump, to stop writers
    --allow-live-source   Convert without stopping Szurubooru. Required instead of -y for an
                          unattended run with no --quiesce-cmd; uploads may be lost.
    --allow-restore-errors  Continue when psql reports errors while restoring the dump.
                          Required instead of -y for an unattended run; whatever those
                          statements carried is missing from the converted database.
    --no-server-env-check Skip the server-container/--oxi-db consistency check, for the case
                          where both really do reach one server under different names.
    --no-single-transaction   Allow a partial database conversion on error
    --keep-dump           Do not delete backup.sql when finished
    --dry-run             Print every command that would run; read-only checks still execute
    --force               Allow resuming into a non-idempotent step
    --force-unpickle      Convert even though the pickled-column probe failed. Separate from
                          --force on purpose: resuming a step must not disarm this guard.
    -y, --yes             Do not prompt for confirmation
    -h, --help            Show this message

Example:
    $0 --oxi-dir /srv/oxibooru \\
       --szuru-db url:postgresql://szuru@db.lan:5432/szuruboru \\
       --oxi-db url:postgresql://oxi@db.lan:5432/oxibooru \\
       --superuser-db url:postgresql://postgres@db.lan:5432/oxibooru \\
       --oxi-server docker:oxibooru-server-1 \\
       --szuru-data /tank/szuru/data --oxi-data /tank/oxi/data --link-data
EOF
    exit "${1:-1}"
}

# -----------------------------------------------------------------------------
# Argument parsing
# -----------------------------------------------------------------------------

SZURU_DATA_OVERRIDE=""
OXI_DATA_OVERRIDE=""

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --szuru-dir)     SZURU_DIR="$2"; shift 2 ;;
            --oxi-dir)       OXI_DIR="$2"; shift 2 ;;
            --convert-sql)   CONVERT_SQL_OVERRIDE="$2"; shift 2 ;;
            --oxi-client)    OXI_CLIENT_CONTAINER="${2#docker:}"; shift 2 ;;
            --szuru-data)    SZURU_DATA_OVERRIDE="$2"; shift 2 ;;
            --oxi-data)      OXI_DATA_OVERRIDE="$2"; shift 2 ;;
            --work-dir)      WORK_DIR="$2"; shift 2 ;;
            --szuru-db)      SRC_DB_SPEC="$2"; shift 2 ;;
            --oxi-db)        TGT_DB_SPEC="$2"; shift 2 ;;
            --superuser-db)  SU_DB_SPEC="$2"; shift 2 ;;
            --oxi-server)    OXI_SERVER_SPEC="$2"; shift 2 ;;
            --copy-data)     DATA_MODE="copy"; shift ;;
            --move-data)     DATA_MODE="move"; shift ;;
            --link-data)     DATA_MODE="link"; shift ;;
            --from)          FROM_STEP="$2"; shift 2 ;;
            --to)            TO_STEP="$2"; shift 2 ;;
            --only)          ONLY_GIVEN=yes; IFS=',' read -r -a ONLY_STEPS <<< "$2"; shift 2 ;;
            --quiesce-cmd)   QUIESCE_CMD="$2"; shift 2 ;;
            --no-single-transaction) SINGLE_TRANSACTION=false; shift ;;
            --keep-dump)     KEEP_DUMP=true; shift ;;
            --dry-run)       DRY_RUN=true; shift ;;
            --force)         FORCE=true; shift ;;
            --force-unpickle) FORCE_UNPICKLE=true; shift ;;
            --allow-live-source)   ALLOW_LIVE_SOURCE=true; shift ;;
            --allow-restore-errors) ALLOW_RESTORE_ERRORS=true; shift ;;
            --no-server-env-check) SKIP_SERVER_ENV_CHECK=true; shift ;;
            -y|--yes)        ASSUME_YES=true; shift ;;
            -h|--help)       usage 0 ;;
            # Upstream's flag name for the server container. The two SQL
            # container flags are gone with docker-addressed databases.
            --oxi-server-container) OXI_SERVER_SPEC="docker:$2"; shift 2 ;;
            *) print_error "Unknown option: $1"; usage ;;
        esac
    done
}
# -----------------------------------------------------------------------------
# Resolve configuration
# -----------------------------------------------------------------------------

resolve_config() {
    if [[ ${#ONLY_STEPS[@]} -gt 0 && ( -n "$FROM_STEP" || -n "$TO_STEP" ) ]]; then
        die "--only is mutually exclusive with --from/--to"
    fi

    FROM_STEP="${FROM_STEP:-${STEPS[0]}}"
    TO_STEP="${TO_STEP:-${STEPS[-1]}}"
    step_index "$FROM_STEP" >/dev/null
    step_index "$TO_STEP" >/dev/null

    # A transposed pair used to run nothing at all and exit 0, which reads exactly
    # like a successful migration.
    if (( $(step_index "$FROM_STEP") > $(step_index "$TO_STEP") )); then
        die "--from '$FROM_STEP' comes after --to '$TO_STEP'; nothing would run."
    fi

    # `--only ""` produced a zero-length array, which fell through to the default
    # preflight..signatures range -- i.e. `--only "$STEPS"` with STEPS unset ran the
    # entire destructive migration, and skipped the --force guard on the way (that
    # guard only iterates a non-empty ONLY_STEPS).
    if [[ -n "$ONLY_GIVEN" ]]; then
        only_valid=false
        for s in "${ONLY_STEPS[@]:-}"; do
            if [[ -n "$s" ]]; then
                step_index "$s" >/dev/null
                only_valid=true
            fi
        done
        [[ "$only_valid" == true ]] || die "--only was given but names no steps. Valid steps: ${STEPS[*]}"
    fi

    # Read .env files where directories were supplied. These provide the upstream
    # defaults; explicit connection specs override them.
    SZURU_ENV_USER=""; SZURU_ENV_DB=""; SZURU_ENV_PASSWORD=""
    OXI_ENV_USER="";   OXI_ENV_DB="";   OXI_ENV_PASSWORD=""

    if [[ -n "$SZURU_DIR" ]]; then
        [[ -d "$SZURU_DIR" ]] || die "Szurubooru directory does not exist: $SZURU_DIR"
        if [[ -f "$SZURU_DIR/.env" ]]; then
            SZURU_ENV_USER="$(read_env_var "$SZURU_DIR/.env" POSTGRES_USER)"
            SZURU_ENV_DB="$(read_env_var "$SZURU_DIR/.env" POSTGRES_DB)"
            SZURU_ENV_PASSWORD="$(read_env_secret "$SZURU_DIR/.env" POSTGRES_PASSWORD)"
            # Resolved against the directory that .env lives in, the way
            # docker resolves a relative MOUNT_DATA.
            SZURU_MOUNT_DATA="$(abs_path "$(read_env_var "$SZURU_DIR/.env" MOUNT_DATA)" "$SZURU_DIR")"
        fi
    fi

    if [[ -n "$OXI_DIR" ]]; then
        [[ -d "$OXI_DIR" ]] || die "Oxibooru directory does not exist: $OXI_DIR"
        if [[ -f "$OXI_DIR/.env" ]]; then
            OXI_ENV_USER="$(read_env_var "$OXI_DIR/.env" POSTGRES_USER)"
            OXI_ENV_DB="$(read_env_var "$OXI_DIR/.env" POSTGRES_DB)"
            OXI_ENV_PASSWORD="$(read_env_secret "$OXI_DIR/.env" POSTGRES_PASSWORD)"
            OXI_MOUNT_DATA="$(abs_path "$(read_env_var "$OXI_DIR/.env" MOUNT_DATA)" "$OXI_DIR")"
        fi
    fi

    # A path typed on the command line is relative to where it was typed.
    [[ -n "$SZURU_DATA_OVERRIDE" ]] && SZURU_MOUNT_DATA="$(abs_path "$SZURU_DATA_OVERRIDE")"
    [[ -n "$OXI_DATA_OVERRIDE" ]] && OXI_MOUNT_DATA="$(abs_path "$OXI_DATA_OVERRIDE")"

    [[ -n "$SRC_DB_SPEC" ]] || die "--szuru-db is required: url:postgresql://user@host:port/dbname"
    [[ -n "$TGT_DB_SPEC" ]] || die "--oxi-db is required: url:postgresql://user@host:port/dbname"
    OXI_SERVER_SPEC="${OXI_SERVER_SPEC:-docker:${ENV_OXI_SERVER_CONTAINER:-oxibooru-server-1}}"

    parse_db_spec SRC "$SRC_DB_SPEC"
    parse_db_spec TGT "$TGT_DB_SPEC"
    parse_server_spec "$OXI_SERVER_SPEC"
    if [[ -n "$SU_DB_SPEC" ]]; then
        parse_db_spec SU "$SU_DB_SPEC"
    fi

    # A URL may carry no password; fall back to .env. No PGPASSWORD fallback: one
    # ambient password cannot be right for three different roles.
    if [[ -z "$SRC_DB_PASSWORD" ]]; then
        SRC_DB_PASSWORD="$SZURU_ENV_PASSWORD"
    fi
    if [[ -z "$TGT_DB_PASSWORD" ]]; then
        TGT_DB_PASSWORD="$OXI_ENV_PASSWORD"
    fi

    WORK_DIR="${WORK_DIR:-${OXI_DIR:-$PWD}}"
    DUMP_FILE="$WORK_DIR/backup.sql"
    # --convert-sql decouples the one file the conversion needs from the oxibooru
    # checkout. --oxi-dir still works and still defaults it, so upstream's
    # invocation is unchanged; but nothing else requires a checkout any more.
    if [[ -n "$CONVERT_SQL_OVERRIDE" ]]; then
        CONVERT_SQL="$CONVERT_SQL_OVERRIDE"
    else
        CONVERT_SQL="${OXI_DIR:+$OXI_DIR/scripts/convert_szuru_database.sql}"
    fi

    print_info "Configuration:"
    echo "  Szurubooru database:      $(db_label SRC)"
    echo "  Oxibooru database:        $(db_label TGT)"
    if [[ -n "$SU_DB_SPEC" ]]; then
        echo "  Superuser connection:     $(db_label SU)"
    fi
    echo "  Oxibooru server:          ${OXI_SERVER_MODE}:${OXI_SERVER_CONTAINER}${OXI_SERVER_BIN}"
    echo "  Szurubooru data:          ${SZURU_MOUNT_DATA:-(not set)}"
    echo "  Oxibooru data:            ${OXI_MOUNT_DATA:-(not set)}"
    echo "  Data mode:                $DATA_MODE"
    echo "  Work directory:           $WORK_DIR"
    echo "  Single transaction:       $SINGLE_TRANSACTION"
    if [[ ${#ONLY_STEPS[@]} -gt 0 ]]; then
        echo "  Steps:                    ${ONLY_STEPS[*]} (--only)"
    else
        echo "  Steps:                    $FROM_STEP .. $TO_STEP"
    fi
    if [[ "$DRY_RUN" == true ]]; then
        echo "  Mode:                     DRY RUN (no changes will be made)"
    fi
    echo ""
}

# -----------------------------------------------------------------------------
# Step: preflight
# -----------------------------------------------------------------------------

# Under --dry-run an unreachable environment is a warning, not a failure, so the
# command plan can still be reviewed off-host.
preflight_fail() {
    if [[ "$DRY_RUN" == true ]]; then
        print_warning "$1"
        return 0
    fi
    die "$1"
}

need_docker() { [[ "$OXI_SERVER_MODE" == "docker" ]]; }

# Whether plpython3u can actually be created. `pg_available_extensions` is not this
# test: it lists plpython3u whenever the .control file is present, even when CREATE
# EXTENSION fails on a missing libpython3.so. Superuser-only, so a 'permission
# denied' answer is inconclusive -- PLPYTHON_PROBE_ERR lets callers tell them apart.
PLPYTHON_PROBE_ERR=""
plpython3u_usable() {
    local role="${1:-TGT}" status=0 raw=""
    build_db_cmd "$role" psql -tAq -v ON_ERROR_STOP=1 \
        -c "BEGIN; CREATE EXTENSION IF NOT EXISTS plpython3u; ROLLBACK;"
    raw="$(with_pgpassword "$role" run_ro "${DB_CMD[@]}" 2>&1 >/dev/null)" || status=$?
    # run_ro echoes the command it is about to run on stderr under --dry-run.
    # That is not a diagnostic, and quoting it back as one is confusing.
    PLPYTHON_PROBE_ERR="$(printf '%s\n' "$raw" | grep -v '^\[dry-run' || true)"
    return "$status"
}

# True when the last probe failed only for lack of superuser, i.e. the result
# was inconclusive rather than negative.
plpython3u_probe_was_denied() {
    [[ "$PLPYTHON_PROBE_ERR" == *"permission denied"* \
    || "$PLPYTHON_PROBE_ERR" == *"must be superuser"* ]]
}

# Poll until the database answers, rather than probing once. A container that
# was just started is not ready for several seconds, and on a stack that has
# never run, the postgres entrypoint has a database to initialise first.
wait_db_reachable() {
    local role="$1"
    # Under --dry-run the environment may not exist at all; waiting a minute to
    # confirm that is not a service to anybody reviewing a plan off-host.
    if [[ "$DRY_RUN" == true ]]; then
        db_reachable "$role"
        return
    fi
    local deadline=$(( SECONDS + DB_READY_TIMEOUT ))
    while :; do
        if db_reachable "$role"; then
            return 0
        fi
        if (( SECONDS >= deadline )); then
            return 1
        fi
        sleep 2
    done
}



# -----------------------------------------------------------------------------
# Oxibooru server container lifecycle
#
# preflight stops it, init starts and stops it, the admin steps need it up. All of
# them go through these two functions so SERVER_STOPPED_BY_SCRIPT stays honest.

# True when this run leaves the database between restore and convert: 'public'
# holding the Szurubooru dump, the Oxibooru schema parked in 'oxi'. Nothing may
# start the server in that state -- see the cleanup block.
conversion_left_unfinished() {
    should_run restore && ! should_run convert
}

server_is_running() {
    [[ "$OXI_SERVER_MODE" == "docker" ]] && command -v docker >/dev/null 2>&1 \
        && [[ "$(docker inspect -f '{{.State.Running}}' "$OXI_SERVER_CONTAINER" 2>/dev/null)" == "true" ]]
}

# `docker start` by name rather than `docker compose up`: no compose file needed,
# and it cannot start a stack's sql service by accident.
start_oxi_stack() {
    if [[ "$OXI_SERVER_MODE" != "docker" ]]; then
        return 0
    fi
    if [[ "$DRY_RUN" != true ]] && server_is_running; then
        SERVER_STOPPED_BY_SCRIPT=false
        return 0
    fi
    print_step "$CURRENT_STEP" "Starting the Oxibooru server container"
    run docker start "$OXI_SERVER_CONTAINER"
    # No longer down at our hands, so the exit path must stop offering to start
    # it. Clearing this used to be the caller's job, and only one caller did it.
    if [[ "$DRY_RUN" != true ]]; then
        SERVER_STOPPED_BY_SCRIPT=false
    fi
}

# Stop it, and remember that we did.
stop_oxi_server() {
    local reason="$1"
    if [[ "$OXI_SERVER_MODE" != "docker" ]]; then
        return 0
    fi
    if [[ "$DRY_RUN" != true ]] && ! server_is_running; then
        return 0
    fi
    print_step "$CURRENT_STEP" "$reason"
    run docker stop "$OXI_SERVER_CONTAINER" >/dev/null
    if [[ "$DRY_RUN" != true ]]; then
        SERVER_STOPPED_BY_SCRIPT=true
    fi
}

# pg_dump's plain-text output ends with a marker line. Its absence means the
# dump was truncated -- a killed pg_dump, a full disk, a dropped connection --
# and restoring it silently loses whatever came after the cut.
dump_looks_complete() {
    local file="$1" tail_lines
    [[ -s "$file" ]] || return 1
    # Ten lines: PostgreSQL 17.6+/18 emit `\unrestrict` after the marker. Captured,
    # not piped to grep -q, whose SIGPIPE would read as 'no marker' under pipefail.
    tail_lines="$(tail -n 10 "$file")"
    [[ "$tail_lines" == *"PostgreSQL database dump complete"* ]]
}

note_server_state() {
    # Observed before anything is started or stopped: the exit path has to tell
    # 'we took a live instance down' from 'it was already down'.
    if server_is_running; then
        SERVER_WAS_RUNNING=true
    fi

}

# The binaries this run needs, in the PATH it will use them from.
check_tools() {
    if need_docker && ! command -v docker >/dev/null 2>&1; then
        preflight_fail "A docker: spec was used but docker is not in PATH"
        PREFLIGHT_OK=false
    fi
    command -v psql >/dev/null 2>&1 || preflight_fail "psql is not in PATH; this script reaches every database with it"
    if should_run dump; then
        command -v pg_dump >/dev/null 2>&1 || preflight_fail "pg_dump is not in PATH; the dump step needs it"
    fi
}

# Every database this run touches, reachable -- and started first, in docker mode.
check_connectivity() {
    if should_run dump; then
        if wait_db_reachable SRC; then
            print_step OK "Szurubooru database reachable ($(db_label SRC))"
        else
            preflight_fail "Cannot reach the Szurubooru database ($(db_label SRC))"
            PREFLIGHT_OK=false
        fi
    fi
    # init belongs here too: it ends by querying TGT for the diesel migrations
    # table, so an unreachable target used to surface as a raw psql error after
    # the container start and the 5s wait, rather than in preflight.
    if should_run restore || should_run convert || should_run init; then
        if wait_db_reachable TGT; then
            print_step OK "Oxibooru database reachable ($(db_label TGT))"
        else
            preflight_fail "Cannot reach the Oxibooru database ($(db_label TGT))"
            PREFLIGHT_OK=false
        fi
    fi

}

# The --superuser-db connection, and that it really authenticates as a superuser.
check_superuser_connection() {
    # The superuser connection was the one connection nothing validated. It is
    # first used at the top of step_convert -- after data, dump, init and the
    # committed schema rename -- so a typo in its host or a role that turns out
    # not to be a superuser cost the whole run and a database drop.
    if [[ -n "$SU_DB_SPEC" ]] && should_run convert; then
        if wait_db_reachable SU; then
            local su_is_super
            su_is_super="$(db_query SU "SELECT rolsuper FROM pg_roles WHERE rolname = current_user")"
            if [[ "$su_is_super" == "t" ]]; then
                print_step OK "Superuser connection reachable and is a superuser ($(db_label SU))"
            else
                preflight_fail "The --superuser-db connection authenticates as a role that is not a SUPERUSER. It cannot grant SUPERUSER to the Oxibooru role, and CREATE EXTENSION plpython3u needs one."
            fi
        else
            preflight_fail "Cannot reach the superuser database ($(db_label SU))"
            PREFLIGHT_OK=false
        fi
    fi

}

# A pg_dump at least as new as the server it is pointed at.
check_dump_client_version() {
    # A dump taken with an older client than the server will fail.
    if should_run dump && [[ "$PREFLIGHT_OK" == true ]] && db_reachable SRC; then
        local server_major client_major
        server_major="$(db_major_version SRC)"
        client_major="$(pg_dump --version | sed -E 's/.* ([0-9]+).*/\1/')"
        if (( client_major < server_major )); then
            preflight_fail "pg_dump is version $client_major but the server is $server_major; install a client >= $server_major"
        else
            print_step OK "pg_dump $client_major can dump server $server_major"
        fi
    fi

}

# The wrong states a target database can already be in before restore.
check_target_schema_state() {
    # Target database state. Three distinct wrong states, all of which otherwise
    # surface long after `restore` has committed the schema rename.
    if should_run restore && db_reachable TGT; then
        local has_alembic has_oxi_schema has_migrations
        has_alembic="$(db_query TGT "SELECT to_regclass('public.alembic_version') IS NOT NULL")"
        if [[ "$has_alembic" == "t" ]]; then
            preflight_fail "The Oxibooru database already contains a Szurubooru schema (public.alembic_version exists). Restoring again would stack two dumps."
        fi

        # An 'oxi' schema means a conversion already ran or half-ran here.
        has_oxi_schema="$(db_query TGT "SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'oxi')")"
        if [[ "$has_oxi_schema" == "t" ]]; then
            preflight_fail "The Oxibooru database already has an 'oxi' schema: a conversion has already run (or partly run) here. Use a fresh database."
        fi

        # A conversion that FINISHED leaves neither marker: it ends with DROP SCHEMA
        # public CASCADE; ALTER SCHEMA oxi RENAME TO public. The data is the only
        # evidence left, and restore wants the Oxibooru schema present and empty.
        local target_populated
        target_populated="$(db_query TGT "
            SELECT CASE
                WHEN to_regclass('public.post') IS NULL THEN 'no-table'
                WHEN EXISTS (SELECT 1 FROM public.post) THEN 'populated'
                ELSE 'empty'
            END")"
        if [[ "$target_populated" == "populated" ]]; then
            preflight_fail "$(cat <<EOF
The Oxibooru database already has posts in it, so this is not a fresh target: a
conversion has already completed here (it renames 'oxi' back to 'public' on the
way out, which is why the checks above cannot see it).
Restoring again would move that converted schema aside and start over on top of
it. Use a fresh database, or drop and recreate this one and resume with
--from init --force.
EOF
)"
        fi

        # restore's first statement renames 'public', which needs ownership. On PG 15+
        # it is owned by pg_database_owner, so a merely-granted role fails here.
        local can_rename
        can_rename="$(db_query TGT "
            SELECT pg_catalog.pg_has_role(current_user, n.nspowner, 'USAGE')
                OR EXISTS (SELECT 1 FROM pg_roles WHERE rolname = current_user AND rolsuper)
            FROM pg_namespace n WHERE n.nspname = 'public'")"
        if [[ "$can_rename" != "t" ]]; then
            resolve_tgt_role
            preflight_fail "$(cat <<EOF
Role '$TGT_DB_ROLE' does not own the 'public' schema, so restore's first statement
(ALTER SCHEMA public RENAME TO oxi) will fail. On PostgreSQL 15+ 'public' is owned by
pg_database_owner. Fix it as a superuser (or the database owner):
    ALTER SCHEMA public OWNER TO "$TGT_DB_ROLE";
EOF
)"
        else
            print_step OK "Oxibooru role can rename the 'public' schema"
        fi

        # Diesel's migrations must already be applied, because `restore` renames
        # public to oxi and convert_szuru_database.sql then writes into oxi.*.
        # Renaming an empty public schema succeeds and only fails ~30 minutes
        # later, on `ALTER TABLE oxi."user" DISABLE TRIGGER USER`.
        if ! should_run init; then
            has_migrations="$(db_query TGT "SELECT to_regclass('public.__diesel_schema_migrations') IS NOT NULL")"
            if [[ "$has_migrations" != "t" ]]; then
                preflight_fail "The Oxibooru database has no migrated schema (no public.__diesel_schema_migrations). Run the init step, or start the oxibooru server once so diesel applies its migrations."
            else
                print_step OK "Oxibooru migrations are already applied"
            fi
        fi
    fi

}

# Superuser rights, and the untrusted language the conversion is written in.
check_plpython3u() {
    # Superuser, needed for CREATE EXTENSION plpython3u.
    if should_run convert && db_reachable TGT; then
        resolve_tgt_role
        local is_super
        is_super="$(db_query TGT "SELECT rolsuper FROM pg_roles WHERE rolname = current_user")"
        if [[ "$is_super" == "t" && -n "$SU_DB_SPEC" ]]; then
            # This script only ever grants SUPERUSER temporarily, so finding it
            # already set alongside --superuser-db usually means a previous run
            # was killed (SIGKILL/OOM) between the grant and the revoke.
            print_warning "Role '$TGT_DB_ROLE' is ALREADY a superuser and --superuser-db was given."
            print_warning "If a previous run was killed mid-conversion, that grant may be left over. Revoke it manually when the migration is done:"
            print_warning "    ALTER ROLE \"$TGT_DB_ROLE\" NOSUPERUSER;"
        elif [[ "$is_super" == "t" ]]; then
            print_step OK "Oxibooru role '$TGT_DB_ROLE' is a superuser"
        elif [[ -n "$SU_DB_SPEC" ]]; then
            print_step OK "Oxibooru role '$TGT_DB_ROLE' is not a superuser; --superuser-db will grant it around the convert step"
        else
            preflight_fail "$(cat <<EOF
Role '$TGT_DB_ROLE' is not a superuser, but CREATE EXTENSION plpython3u is untrusted DDL and requires one.
Either pass --superuser-db url:postgresql://postgres@<host>:<port>/<db> so this script can grant and revoke it,
or do it yourself:
    ALTER ROLE "$TGT_DB_ROLE" SUPERUSER;    -- before
    ALTER ROLE "$TGT_DB_ROLE" NOSUPERUSER;  -- after
EOF
)"
        fi

        # convert_szuru_database.sql:5 is a bare CREATE EXTENSION with no
        # IF NOT EXISTS, so an extension already installed here is a hard failure
        # a few seconds into the conversion.
        local already_installed
        already_installed="$(db_query TGT "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'plpython3u')")"
        if [[ "$already_installed" == "t" ]]; then
            preflight_fail "plpython3u is already installed in this database, but convert_szuru_database.sql does a bare CREATE EXTENSION and will fail. Drop it first: DROP EXTENSION plpython3u;"
        fi

        # CREATE EXTENSION is superuser-only and the grant happens in convert, so probe
        # as the superuser where it addresses the same database -- same server AND name,
        # since extensions are per-database.
        local probe_role="TGT"
        if [[ -n "$SU_DB_SPEC" ]]; then
            if same_server SU TGT && [[ "$SU_DB_NAME" == "$TGT_DB_NAME" ]]; then
                probe_role="SU"
            elif ! same_server SU TGT; then
                # Not fatal: one server can be reached by two routes. But the grant lands
                # where --superuser-db points, and the conversion runs on --oxi-db.
                print_warning "Cannot confirm that --superuser-db and --oxi-db address the same PostgreSQL server:"
                print_warning "    --superuser-db: $(db_label SU)"
                print_warning "    --oxi-db:       $(db_label TGT)"
                print_warning "The SUPERUSER grant is issued on the first and the conversion runs on the second. If they are not the same server, the grant will not apply to the conversion."
            fi
        fi

        if plpython3u_usable "$probe_role"; then
            print_step OK "plpython3u can be created on the server"
        elif plpython3u_probe_was_denied; then
            print_warning "$(cat <<EOF
Could not verify plpython3u: CREATE EXTENSION was refused for lack of superuser. That is expected
here and says nothing about whether the package is installed. To settle it at preflight, point
--superuser-db at '$TGT_DB_NAME' itself; otherwise the convert step finds out after it grants
SUPERUSER.
EOF
)"
        else
            local major
            major="$(db_major_version TGT)"
            preflight_fail "$(cat <<EOF
plpython3u is not available on the Oxibooru server, and this script cannot install packages on a host
it only holds a database connection to. The probe reported:
    ${PLPYTHON_PROBE_ERR:-(no error text)}
Install it on the database host for PostgreSQL $major:
    Debian/Ubuntu (PGDG):  apt-get install postgresql-plpython3-${major} python3-sqlalchemy
    RHEL/Rocky (PGDG):     dnf install postgresql${major}-plpython3 python3-sqlalchemy
    Alpine:                apk add postgresql${major}-plpython3 py3-sqlalchemy
Then re-run with --from convert.
EOF
)"
        fi
    fi

}

# The conversion SQL itself.
check_convert_sql() {
    # The conversion SQL comes from the oxibooru checkout, unmodified.
    if should_run convert; then
        if [[ -z "$CONVERT_SQL" ]]; then
            preflight_fail "--oxi-dir is required for the convert step (it holds scripts/convert_szuru_database.sql)"
        elif [[ ! -f "$CONVERT_SQL" ]]; then
            preflight_fail "Not found: $CONVERT_SQL"
        else
            print_step OK "Found $CONVERT_SQL"
        fi
    fi

}

# Source and destination data directories, and what --link-data needs of them.
check_data_dirs() {
    # Data directories.
    if should_run data; then
        [[ -n "$SZURU_MOUNT_DATA" ]] || preflight_fail "Szurubooru data directory unknown (set MOUNT_DATA in .env or pass --szuru-data)"
        [[ -n "$OXI_MOUNT_DATA" ]] || preflight_fail "Oxibooru data directory unknown (set MOUNT_DATA in .env or pass --oxi-data)"
        if [[ -n "$SZURU_MOUNT_DATA" && ! -d "$SZURU_MOUNT_DATA" ]]; then
            preflight_fail "Szurubooru data directory does not exist: $SZURU_MOUNT_DATA"
        fi
        if [[ -n "$OXI_MOUNT_DATA" && -e "$OXI_MOUNT_DATA" ]]; then
            preflight_fail "Oxibooru data directory already exists: $OXI_MOUNT_DATA (this step expects a fresh installation)"
        fi
        if [[ "$DATA_MODE" == "link" ]]; then
            # Not `cp --help | grep -q`: grep exits at the first match and can
            # SIGPIPE cp, which under `set -o pipefail` makes the pipeline 141
            # and turns a passing check into a spurious failure.
            local cp_help
            cp_help="$(cp --help 2>&1 || true)"
            if [[ "$cp_help" != *--link* ]]; then
                preflight_fail "--link-data needs GNU cp (cp -al)"
            fi

            # Walk up to the nearest ancestor that exists: step_data creates the
            # parent when it is missing, so gating on -d "$dst_parent" would skip
            # the check entirely on exactly the fresh installs it matters for.
            local src_dev dst_anc dst_dev
            dst_anc="$(dirname "$OXI_MOUNT_DATA")"
            while [[ -n "$dst_anc" && "$dst_anc" != "/" && "$dst_anc" != "." && ! -d "$dst_anc" ]]; do
                dst_anc="$(dirname "$dst_anc")"
            done
            if [[ -d "$SZURU_MOUNT_DATA" && -d "$dst_anc" ]]; then
                src_dev="$(stat -c %d "$SZURU_MOUNT_DATA")"
                dst_dev="$(stat -c %d "$dst_anc")"
                if [[ "$src_dev" != "$dst_dev" ]]; then
                    preflight_fail "--link-data requires the same filesystem: $SZURU_MOUNT_DATA and $dst_anc are on different devices"
                else
                    print_step OK "Hard links possible between $SZURU_MOUNT_DATA and $dst_anc"
                fi
            fi

            # A nested mount makes cp -al fail partway and leaves a half-linked tree
            # that blocks the retry. GNU find's %D walks the whole tree for one.
            local crossing sub sub_dev find_version
            find_version="$(find --version 2>/dev/null || true)"
            if [[ -d "$SZURU_MOUNT_DATA" && -n "${src_dev:-}" && "$find_version" == *GNU* ]]; then
                crossing="$(find "$SZURU_MOUNT_DATA" -type d -printf '%D %p\n' 2>/dev/null \
                            | awk -v dev="$src_dev" '$1 != dev {print $2; exit}' || true)"
                if [[ -n "$crossing" ]]; then
                    preflight_fail "--link-data cannot cross the mount at ${crossing}: it is a different filesystem from $SZURU_MOUNT_DATA"
                else
                    print_step OK "No nested mounts under $SZURU_MOUNT_DATA"
                fi
            else
                for sub in "$SZURU_MOUNT_DATA"/*/; do
                    if [[ -d "$sub" ]]; then
                        sub_dev="$(stat -c %d "$sub")"
                        if [[ -n "${src_dev:-}" && "$sub_dev" != "$src_dev" ]]; then
                            preflight_fail "--link-data cannot cross the mount at ${sub}: it is a different filesystem from $SZURU_MOUNT_DATA"
                        fi
                    fi
                done
                print_warning "GNU find is not available: only the top level of $SZURU_MOUNT_DATA was checked for nested mounts, so cp -al may still fail deeper in the tree."
            fi
        fi
    fi

}

# Somewhere to write backup.sql -- or a complete one already sitting there.
check_work_dir() {
    # Writable work directory for the dump.
    if should_run dump; then
        [[ -d "$WORK_DIR" ]] || preflight_fail "Work directory does not exist: $WORK_DIR"
        [[ -w "$WORK_DIR" ]] || preflight_fail "Work directory is not writable: $WORK_DIR"
    fi
    if should_run restore && ! should_run dump; then
        if [[ ! -f "$DUMP_FILE" ]]; then
            preflight_fail "No dump to restore at $DUMP_FILE (run the dump step, or pass --work-dir)"
        elif ! dump_looks_complete "$DUMP_FILE"; then
            # Existence was the only test here, so a dump left truncated by a
            # killed pg_dump restored as if it were whole: the tables it did
            # contain passed the public.post check, and the missing tail only
            # surfaced as posts nobody could find.
            preflight_fail "$(cat <<EOF
$DUMP_FILE is truncated: it does not end with pg_dump's completion marker, so the
dump was interrupted. Restoring it would silently lose everything after the cut.
Re-run the dump step, or delete the file and pass --from dump.
EOF
)"
        else
            print_step OK "Dump at $DUMP_FILE is complete"
        fi
    fi

}

# Whether the server container and --oxi-db agree on what they address.
check_server_container_env() {
    # The oxibooru server is configured entirely separately from this script:
    # it reads its own environment, and an admin task is just a task name piped
    # into `./server --admin`. If the two disagree, the conversion writes to one
    # database while reset_filenames and recompute_signatures fix up another --
    # silently, and only discovered by the results being wrong. The container
    # knows both answers, so ask it rather than asking the operator to be careful.
    if [[ "$OXI_SERVER_MODE" == "docker" ]] && [[ "$SKIP_SERVER_ENV_CHECK" != true ]] \
       && command -v docker >/dev/null 2>&1 \
       && docker inspect "$OXI_SERVER_CONTAINER" >/dev/null 2>&1; then
        local c_env c_db c_host c_port c_data
        # One inspect, then read the variables out of it. `... | sed | head -n1`
        # was a pipeline whose SIGPIPE could abort preflight under pipefail.
        c_env="$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$OXI_SERVER_CONTAINER" 2>/dev/null || true)"
        c_db="$(printf '%s\n' "$c_env" | sed -n 's/^POSTGRES_DB=//p' | tail -n1)"
        c_host="$(printf '%s\n' "$c_env" | sed -n 's/^POSTGRES_HOST=//p' | tail -n1)"
        c_port="$(printf '%s\n' "$c_env" | sed -n 's/^POSTGRES_PORT=//p' | tail -n1)"
        c_data="$(docker inspect -f '{{range .Mounts}}{{if eq .Destination "/data"}}{{.Source}}{{end}}{{end}}' "$OXI_SERVER_CONTAINER" 2>/dev/null)"

        if [[ -n "$c_db" && -n "$TGT_DB_NAME" && "$c_db" != "$TGT_DB_NAME" ]]; then
            preflight_fail "$(cat <<EOF
The Oxibooru server container and --oxi-db name different databases:
    container:  $c_db
    --oxi-db:   $TGT_DB_NAME
The conversion would write to one and the admin tasks would fix up the other.
Point the server's environment at '$TGT_DB_NAME' and recreate the container.
EOF
)"
        elif [[ -n "$c_db" ]]; then
            print_step OK "Server container targets database '$c_db', matching --oxi-db"
        elif [[ -n "$TGT_DB_NAME" ]]; then
            # Silence here used to read as agreement. It is not: POSTGRES_DB has
            # no default in the server (server/src/config.rs reads it with `?`),
            # so either it arrives from a .env mounted inside the container --
            # which docker inspect cannot see, and which this script therefore
            # cannot check -- or the server will not start at all.
            print_warning "The Oxibooru server container sets no POSTGRES_DB in its environment, so this check cannot confirm it targets '$TGT_DB_NAME'. If it reads one from a .env inside the container, make sure that names the same database; otherwise the server will not start."
        fi

        # The database NAME matching proves nothing on its own: docker-compose.yml
        # ships POSTGRES_HOST=sql, so a server container left at that default and
        # an --oxi-db pointing at a remote host agree on 'oxibooru' while
        # addressing two different servers. The conversion then lands on one and
        # reset_filenames/recompute_* on the other -- the exact split-brain this
        # block exists to prevent, reported as [OK].
        # A URL with no host at all (postgresql:///db) means a local socket to
        # libpq, which a container reaching POSTGRES_HOST cannot be. Checked
        # first: the mismatch branch below needs TGT_DB_HOST non-empty, and
        # without this the empty case fell through to the [OK] branch.
        if [[ -n "$c_host" && -z "$TGT_DB_HOST" ]]; then
            preflight_fail "$(cat <<EOF
--oxi-db names no host, so it is a local socket on this machine, while the
Oxibooru server container connects to '$c_host'. Those cannot be the same
server, so the conversion and the admin tasks would land on different databases.
Give --oxi-db the host the container uses, or pass --no-server-env-check.
EOF
)"
        elif [[ -n "$c_host" && -n "$TGT_DB_HOST" && "$c_host" != "$TGT_DB_HOST" ]]; then
            preflight_fail "$(cat <<EOF
The Oxibooru server container and --oxi-db name different database hosts:
    container POSTGRES_HOST:  $c_host
    --oxi-db host:            $TGT_DB_HOST
The conversion would run on one server and the admin tasks (reset_filenames,
recompute_checksums, recompute_signatures) on the other, leaving a converted
database whose posts keep their Szurubooru filenames, checksums and signatures.
Point the server's environment at '$TGT_DB_HOST' and recreate the container.
If the two names really are the same server reached by different routes, pass
--no-server-env-check.
EOF
)"
        elif [[ -n "$c_host" ]]; then
            print_step OK "Server container targets host '$c_host', matching --oxi-db"
            # Effective ports: an omitted port means 5432 to libpq, and to the server
            # (DEFAULT_POSTGRES_PORT in server/src/config.rs).
            local c_port_eff="${c_port:-5432}" tgt_port_eff="${TGT_DB_PORT:-5432}"
            if [[ "$c_port_eff" != "$tgt_port_eff" ]]; then
                preflight_fail "$(cat <<EOF
The Oxibooru server container and --oxi-db name different ports on '$c_host':
    container POSTGRES_PORT:  ${c_port:-(unset, so 5432)}
    --oxi-db port:            ${TGT_DB_PORT:-(unset, so 5432)}
The conversion would run on one server and the admin tasks on the other.
If both really do reach one server, pass --no-server-env-check.
EOF
)"
            else
                print_step OK "Server container and --oxi-db agree on port $tgt_port_eff"
            fi
        elif [[ -z "$c_host" ]]; then
            print_warning "The Oxibooru server container sets no POSTGRES_HOST, so it will use its own default while --oxi-db points at $TGT_DB_HOST. Confirm the admin tasks reach the same database the conversion writes to."
        fi

        if [[ -n "$c_data" && -n "$OXI_MOUNT_DATA" && "$(abs_path "$c_data")" != "$OXI_MOUNT_DATA" ]]; then
            preflight_fail "$(cat <<EOF
The Oxibooru server container and --oxi-data name different data directories:
    container:   $c_data
    --oxi-data:  $OXI_MOUNT_DATA
reset_filenames renames files in whichever tree the container has mounted.
EOF
)"
        elif [[ -n "$c_data" ]]; then
            print_step OK "Server container mounts /data from '$c_data', matching --oxi-data"
        fi
    fi

}

# The Oxibooru server that the four admin steps run inside.
check_server_available() {
    # The four admin steps are the last thing to run and the most expensive to
    # reach; failing at `docker exec` an hour in is avoidable.
    if should_run filenames || should_run thumbsizes || should_run checksums || should_run signatures || should_run init; then
        case "$OXI_SERVER_MODE" in
            docker)
                if command -v docker >/dev/null 2>&1 \
                   && ! docker inspect "$OXI_SERVER_CONTAINER" >/dev/null 2>&1; then
                    preflight_fail "Oxibooru server container not found: $OXI_SERVER_CONTAINER. Create the stack first (docker compose up --no-start, Portainer, whatever you use); this script starts and stops it, but does not create it."
                fi
                ;;
            exec)
                [[ -x "$OXI_SERVER_BIN" ]] || preflight_fail "Oxibooru server binary is not executable: $OXI_SERVER_BIN"
                ;;
        esac
    fi

}

# Whether anything is still writing to Szurubooru.
check_source_quiesced() {
    # Nothing stops Szurubooru writing during the run unless the operator says how,
    # and a post uploaded mid-run gets a row with no file, or no row at all.
    if (should_run data || should_run dump) && [[ -z "$QUIESCE_CMD" && "$ALLOW_LIVE_SOURCE" != true ]]; then
        print_warning "$(cat <<EOF
No --quiesce-cmd given: nothing will stop Szurubooru from writing during the run.
A post uploaded after the data step gets a row in the dump and no file on disk; a post
uploaded after the dump is not carried over at all. Stop Szurubooru's writers first, or
have this script do it:
    --quiesce-cmd 'docker compose -f ${SZURU_DIR:-/path/to/szurubooru}/docker-compose.yml stop client api'
(Leave its sql service up -- the dump reads from it.)
EOF
)"
        # -y must not wave this through: an unattended run would convert a live
        # instance with only a warning. --allow-live-source is the explicit yes.
        if [[ "$ASSUME_YES" == true ]]; then
            die "Refusing to run unattended against a Szurubooru that may still be writing. Pass --quiesce-cmd to stop it, or --allow-live-source to accept the risk."
        fi
        if [[ "$DRY_RUN" != true ]]; then
            confirm "Continue against a Szurubooru instance that may still be running?" \
                || die "Aborted by user"
        fi
    elif (should_run data || should_run dump) && [[ -z "$QUIESCE_CMD" ]]; then
        print_warning "--allow-live-source: proceeding without stopping Szurubooru. Posts uploaded from now on may be lost or left without files."
    fi

}

# Refusals: resuming into a step that assumes state it will not find.
check_resume_allowed() {
    # Resuming into a step that assumes earlier state.
    if [[ "$FROM_STEP" != "${STEPS[0]}" ]] && is_non_idempotent "$FROM_STEP" && [[ "$FORCE" != true ]]; then
        die "Resuming at '$FROM_STEP' re-runs a step that is not idempotent. Re-run with --force if that is what you want."
    fi
    if [[ ${#ONLY_STEPS[@]} -gt 0 && "$FORCE" != true ]]; then
        local s
        for s in "${ONLY_STEPS[@]}"; do
            if is_non_idempotent "$s"; then
                die "Step '$s' is not idempotent. Re-run with --force if that is what you want."
            fi
        done
    fi

}

# Take the server down -- only once every refusal above has passed.
stop_server_for_restore() {
    # restore and convert rewrite the schema under anything connected, so take the
    # server down. LAST in preflight, deliberately: it used to come before the
    # refusals below, so a run the script declined still stopped a live instance.
    if (should_run restore || should_run convert) && server_is_running; then
        stop_oxi_server "Stopping $OXI_SERVER_CONTAINER: restore/convert need the database to themselves"
    elif (should_run restore || should_run convert) && [[ "$OXI_SERVER_MODE" == "exec" ]]; then
        print_warning "Make sure the Oxibooru server ($OXI_SERVER_BIN) is not running: restore and convert rewrite the schema underneath it."
    fi
}

# Preflight is this list of checks, in this order. The order is not cosmetic:
# every refusal has to come before stop_server_for_restore, or a run the script
# then declines to perform still takes the operator's instance down on its way out.
step_preflight() {
    PREFLIGHT_OK=true

    note_server_state
    check_tools
    check_connectivity
    check_superuser_connection
    check_dump_client_version
    check_target_schema_state
    check_plpython3u
    check_convert_sql
    check_data_dirs
    check_work_dir
    check_server_container_env
    check_server_available
    check_source_quiesced
    check_resume_allowed
    stop_server_for_restore

    print_info "Preflight complete."
}

# -----------------------------------------------------------------------------
# Step: data
# -----------------------------------------------------------------------------

quiesce_writers() {
    if [[ -n "$QUIESCE_CMD" && "$QUIESCED" != true ]]; then
        QUIESCED=true
        print_step quiesce "Stopping writers: $QUIESCE_CMD"
        run bash -c "$QUIESCE_CMD"
    fi
}

step_data() {
    # Must happen before the tree is captured, not before the dump. With
    # --link-data the destination is a snapshot of directory entries taken right
    # here; a post uploaded after this and before the dump would get a row in
    # the dump and no file in the oxibooru tree.
    quiesce_writers

    local parent
    parent="$(dirname "$OXI_MOUNT_DATA")"
    if [[ ! -d "$parent" ]]; then
        print_step data "Creating parent directory: $parent"
        run mkdir -p "$parent"
    fi

    case "$DATA_MODE" in
        move)
            print_warning "Moving the data directory. Szurubooru will no longer be able to read it."
            confirm "Continue?" || die "Aborted by user"
            print_step data "Moving $SZURU_MOUNT_DATA -> $OXI_MOUNT_DATA"
            run mv "$SZURU_MOUNT_DATA" "$OXI_MOUNT_DATA"
            ;;
        link)
            print_step data "Hard-linking $SZURU_MOUNT_DATA -> $OXI_MOUNT_DATA"
            run cp -al "$SZURU_MOUNT_DATA" "$OXI_MOUNT_DATA"
            print_info "Trees share inodes. reset_filenames only renames directory entries, so the Szurubooru tree stays intact."
            print_warning "Do NOT run regenerate_thumbnails while the trees are linked: it rewrites file contents in place."
            ;;
        copy)
            print_step data "Copying $SZURU_MOUNT_DATA -> $OXI_MOUNT_DATA"
            run cp -r "$SZURU_MOUNT_DATA" "$OXI_MOUNT_DATA"
            ;;
    esac

    # Oxibooru expects custom thumbnails one level up from where Szurubooru put them.
    local src="$OXI_MOUNT_DATA/posts/custom-thumbnails"
    local dst="$OXI_MOUNT_DATA/custom-thumbnails"
    if [[ -d "$src" ]]; then
        print_step data "Relocating custom thumbnails to $dst"
        run mv "$src" "$dst"
    fi
}

# -----------------------------------------------------------------------------
# Step: dump
# -----------------------------------------------------------------------------

step_dump() {
    # No-op if step_data already did it; this covers --from dump.
    quiesce_writers

    print_step dump "Dumping $(db_label SRC) to $DUMP_FILE"
    if [[ "$DRY_RUN" == true ]]; then
        # The redirection would be performed by this shell before `run` ever saw
        # the dry-run flag, truncating a dump the operator may be about to reuse.
        db_dump SRC
        return 0
    fi

    db_dump SRC > "$DUMP_FILE"

    if [[ ! -s "$DUMP_FILE" ]]; then
        die "Database dump is empty: $DUMP_FILE"
    fi
    # pg_dump can exit non-zero after writing gigabytes, and `set -e` would take
    # us out before this -- but a dump interrupted at the socket level can also
    # end here looking plausible. The marker is the only cheap proof it is whole.
    if ! dump_looks_complete "$DUMP_FILE"; then
        die "Database dump is truncated (no completion marker at the end of $DUMP_FILE). Re-run the dump step."
    fi
    print_info "Dump created: $DUMP_FILE ($(du -h "$DUMP_FILE" | cut -f1))"
}

# -----------------------------------------------------------------------------
# Step: init
# -----------------------------------------------------------------------------

# Poll until the server has created the diesel migrations table, rather than
# sleeping a fixed 5s and hoping. Returns 1 on timeout.
# How many migrations the target has applied. -1 when diesel has not created its
# bookkeeping table yet, which is not the same as zero.
applied_migration_count() {
    local n
    n="$(db_query TGT "
        SELECT CASE
            WHEN to_regclass('public.__diesel_schema_migrations') IS NULL THEN -1
            ELSE (SELECT COUNT(*) FROM public.__diesel_schema_migrations)
        END" 2>/dev/null || true)"
    [[ "$n" =~ ^-?[0-9]+$ ]] || n=-1
    printf '%s' "$n"
}

# Wait for diesel to FINISH. The table's existence is not the finish line: the
# harness creates __diesel_schema_migrations before applying the first migration,
# so init used to stop the server mid-migration. The count is compiled into the
# server image, so wait for it to stop moving rather than for a number.
MIGRATIONS_SEEN=-1

wait_for_migrations() {
    local deadline=$(( SECONDS + MIGRATION_TIMEOUT ))
    local count last=-1 stable=0
    # Confirming that the count has settled costs two poll intervals, so a
    # MIGRATION_TIMEOUT under ~10s cannot succeed however fast the server is.
    while (( SECONDS < deadline )); do
        count="$(applied_migration_count)"
        if (( count > 0 && count == last )); then
            stable=$(( stable + 1 ))
            # Two consecutive quiet polls. One is not enough: it is also what
            # the gap between two migrations looks like.
            (( stable >= 2 )) && return 0
        else
            stable=0
        fi
        last="$count"
        MIGRATIONS_SEEN="$count"
        sleep 2
    done
    return 1
}

step_init() {
    print_step init "Starting the Oxibooru server so it applies its migrations"
    if [[ "$OXI_SERVER_MODE" == "docker" ]]; then
        start_oxi_stack
        if [[ "$DRY_RUN" != true ]]; then
            if ! wait_for_migrations; then
                if (( MIGRATIONS_SEEN > 0 )); then
                    # Not "the server never started" -- it was working, and
                    # stopping the container now is exactly what leaves a
                    # half-migrated schema behind.
                    die "The Oxibooru server was still applying migrations when the clock ran out (${MIGRATIONS_SEEN} applied after ${MIGRATION_TIMEOUT}s, still climbing). Raise MIGRATION_TIMEOUT and re-run; it also has to leave room for two quiet polls, so under ~10s can never succeed."
                fi
                die "The Oxibooru server did not apply its migrations within ${MIGRATION_TIMEOUT}s. Check: docker logs $OXI_SERVER_CONTAINER"
            fi
        fi
        # Through stop_oxi_server, so a run ending here says the instance is down.
        stop_oxi_server "Stopping $OXI_SERVER_CONTAINER: the migrations are applied and convert needs the database to itself"
    else
        # No container to drive. The binary applies migrations at startup, so run
        # it briefly and stop it; there is no --wait equivalent here.
        print_info "Server is a local binary; starting it briefly to apply migrations."
        if [[ "$DRY_RUN" == true ]]; then
            run "$OXI_SERVER_BIN" "(started, then stopped after migrations)"
        else
            "$OXI_SERVER_BIN" &
            local server_pid=$!
            wait_for_migrations || true
            kill "$server_pid" 2>/dev/null || true
            wait "$server_pid" 2>/dev/null || true
        fi
    fi

    if [[ "$DRY_RUN" != true ]]; then
        # The count, not the table: an empty __diesel_schema_migrations means
        # diesel got as far as creating its bookkeeping and no further.
        local migrated
        migrated="$(applied_migration_count)"
        if (( migrated < 1 )); then
            die "The Oxibooru server did not apply its migrations (public.__diesel_schema_migrations is missing or empty). Check the server logs."
        fi
        print_info "Migrations applied ($migrated)."
    fi
}

# -----------------------------------------------------------------------------
# Step: restore
# -----------------------------------------------------------------------------

step_restore() {
    print_step restore "Moving the Oxibooru schema aside (public -> oxi)"
    db_exec TGT -c "ALTER SCHEMA public RENAME TO oxi;"
    # Only past this point is the database changed in a way that needs a drop to
    # undo. resume_hint reads this to decide which recovery to prescribe -- so
    # not under --dry-run, where the ALTER above was printed, not executed.
    if [[ "$DRY_RUN" != true ]]; then
        RESTORE_RENAMED=true
    fi
    db_exec TGT -c "CREATE SCHEMA public;"

    print_step restore "Restoring the Szurubooru dump into public"
    if [[ "$DRY_RUN" == true ]]; then
        db_exec_file TGT "$DUMP_FILE" lenient -o /dev/null
        return 0
    fi

    # Upstream lets psql report errors and carry on. Keep that tolerance, but
    # count what happened instead of letting it scroll past unnoticed.
    local errlog errors has_post psql_status=0
    errlog="$(mktemp)"
    # shellcheck disable=SC2064
    trap "rm -f '$errlog'" RETURN
    db_exec_file TGT "$DUMP_FILE" lenient -o /dev/null 2> "$errlog" || psql_status=$?

    # In lenient mode psql exits 0 for a dump full of failed statements, so a
    # non-zero status means psql itself stopped -- a lost connection prints no
    # ERROR: line at all, and counting them called that restore clean.
    if (( psql_status != 0 )); then
        print_error "psql exited $psql_status while restoring the dump; the restore is INCOMPLETE."
        [[ "$psql_status" == 2 ]] && print_error "(2 means the connection to the server was lost.)"
        tail -n 20 "$errlog" >&2 || true
        die "Refusing to continue with a partially restored database."
    fi

    # psql prefixes diagnostics from a script with `psql:<file>:<line>: `, so an
    # anchored ^ERROR: matched nothing. `.*` because a path may contain a colon.
    local err_re='^(psql:.*:[0-9]+: )?ERROR:'
    errors="$(grep -Ec "$err_re" "$errlog" || true)"
    if (( errors > 0 )); then
        print_warning "psql reported $errors error(s) while restoring:"
        # grep -m 20, not `grep | head`: head's SIGPIPE aborts the step under pipefail.
        grep -m 20 -E "$err_re" "$errlog" >&2 || true
        # -y must not wave this through either: what is missing is whatever those
        # statements carried, and nothing downstream notices.
        if [[ "$ALLOW_RESTORE_ERRORS" == true ]]; then
            print_warning "--allow-restore-errors given; continuing with a restore that reported $errors error(s)."
        elif [[ "$ASSUME_YES" == true ]]; then
            die "Refusing to continue unattended after $errors restore error(s). Read them above; re-run with --allow-restore-errors if the dump really is fine."
        else
            confirm "Continue anyway?" || die "Aborted by user"
        fi
    fi

    # A dump that restored nothing usable is worth catching here rather than
    # three steps later.
    has_post="$(db_query TGT "SELECT to_regclass('public.post') IS NOT NULL")"
    if [[ "$has_post" != "t" ]]; then
        die "Restore did not produce a public.post table; the dump is not a Szurubooru database."
    fi
    print_info "Database restored."
}

# -----------------------------------------------------------------------------
# Step: convert
# -----------------------------------------------------------------------------

# Whether `import sqlalchemy` works server-side: Szurubooru pickles SQLAlchemy
# objects into snapshot.data. Needs the extension, so call it past the grant.
plpython3u_has_sqlalchemy() {
    local out status=0
    out="$(db_query_script TGT "$(cat <<'SQL'
BEGIN;
CREATE EXTENSION IF NOT EXISTS plpython3u;
SET LOCAL client_min_messages = warning;
CREATE FUNCTION pg_temp.probe_sqlalchemy() RETURNS TEXT LANGUAGE plpython3u AS $probe$
import sqlalchemy
return 'sqlalchemy-ok'
$probe$;
SELECT pg_temp.probe_sqlalchemy();
ROLLBACK;
SQL
)" 2>&1)" || status=$?
    (( status == 0 )) && [[ "$out" == *sqlalchemy-ok* ]]
}


ensure_plpython3u() {
    if ! db_usable TGT; then
        print_info "Dry run: skipping the plpython3u availability check (target unreachable)."
        return 0
    fi
    if plpython3u_usable; then
        # Upstream installed SQLAlchemy alongside plpython3u because Szurubooru
        # pickles SQLAlchemy objects into snapshot.data. probe_unpickle below is
        # the authority on whether this database has any.
        plpython3u_has_sqlalchemy || print_warning "plpython3u works, but 'import sqlalchemy' fails on the database host. Install python3-sqlalchemy (py3-sqlalchemy on Alpine) there if the probe below asks for it."
        return 0
    fi
    # Under --dry-run the SUPERUSER grant was printed, not executed, so a
    # permission-denied answer here is an artefact of the rehearsal.
    if [[ "$DRY_RUN" == true ]] && plpython3u_probe_was_denied; then
        print_info "Dry run: cannot confirm plpython3u as '$TGT_DB_ROLE' -- the SUPERUSER grant was not applied."
        return 0
    fi
    die "plpython3u cannot be created, and this script cannot install packages on a host it only holds a database connection to. This probe ran after the SUPERUSER grant, so it is not a permissions problem. psql said: ${PLPYTHON_PROBE_ERR:-(no error text)}"
}

# The role psql actually authenticates as -- not necessarily the URL's user, since
# PGUSER or a service file can decide it. Getting it wrong grants the wrong role.
TGT_DB_ROLE=""
resolve_tgt_role() {
    local r
    if [[ -z "$TGT_DB_ROLE" ]] && db_reachable TGT; then
        r="$(db_query TGT "SELECT current_user")"
        [[ -n "$r" ]] && TGT_DB_ROLE="$r"
    fi
    [[ -n "$TGT_DB_ROLE" ]] || TGT_DB_ROLE="$TGT_DB_USER"
}

grant_superuser_if_needed() {
    if ! db_usable TGT; then
        print_info "Dry run: skipping the superuser check (target unreachable)."
        return 0
    fi
    resolve_tgt_role
    local is_super
    is_super="$(db_query TGT "SELECT rolsuper FROM pg_roles WHERE rolname = current_user")"
    if [[ "$is_super" == "t" ]]; then
        return 0
    fi
    [[ -n "$SU_DB_SPEC" ]] || die "Role '$TGT_DB_ROLE' is not a superuser and no --superuser-db was given."

    print_step convert "Granting SUPERUSER to '$TGT_DB_ROLE' for the duration of the conversion"
    db_exec SU -c "ALTER ROLE \"$TGT_DB_ROLE\" SUPERUSER;"
    SUPERUSER_GRANTED=true

    # Confirm the grant landed on the role psql authenticates as. Not under --dry-run,
    # where the ALTER ROLE was printed rather than executed.
    if [[ "$DRY_RUN" == true ]]; then
        print_info "Dry run: the SUPERUSER grant was printed, not applied; skipping the verification query."
        return 0
    fi
    is_super="$(db_query TGT "SELECT rolsuper FROM pg_roles WHERE rolname = current_user")"
    if [[ "$is_super" != "t" ]]; then
        die "Granted SUPERUSER to '$TGT_DB_ROLE', but the target connection still authenticates as a non-superuser. Check which role --oxi-db actually connects as."
    fi
}

# The conversion unpickles post_note.polygon and snapshot.data server-side.
# Whether that needs anything beyond the stdlib depends on what Szurubooru
# pickled, so find out on a sample before committing to the full run.
#
# The probe mirrors what convert_szuru_database.sql actually does with each
# column, not just pickle.loads: unpickle_to_jsonb ends in json.dumps, so a
# pickled datetime/set/Decimal unpickles fine and then fails an hour later on
# "Object of type X is not JSON serializable". unpickle_to_array flattens the
# polygon into pairs, which fails on a malformed shape the same way.
probe_unpickle() {
    print_step convert "Probing pickled columns before converting"
    local probe_sql failures status=0
    probe_sql=$(cat <<'SQL'
BEGIN;
CREATE EXTENSION IF NOT EXISTS plpython3u;
-- Without this, "extension already exists, skipping" lands on stderr and is
-- read as a probe failure.
SET LOCAL client_min_messages = warning;

-- These two bodies are copied VERBATIM from convert_szuru_database.sql, and the
-- declared return types (REAL[], JSONB) are part of the test: half the ways this
-- conversion fails are Postgres rejecting the *result*, not Python raising.
-- json.dumps happily emits NaN, and a NUL inside a string; JSONB rejects both.
-- A polygon of
-- 1e40 unpickles fine and overflows REAL. Do not "simplify" these to TEXT.
CREATE FUNCTION pg_temp.unpickle_to_array(raw BYTEA) RETURNS REAL[] LANGUAGE plpython3u AS $probe$
import pickle

if hasattr(raw, 'tobytes'):
    data_bytes = raw.tobytes()
else:
    data_bytes = raw

try:
    pts = pickle.loads(data_bytes)
except Exception as e:
    plpy.error(f"Could not unpickle post_note.polygon: {e!s}")

try:
    flat = []
    for p in pts:
        if len(p) != 2:
            raise ValueError(f"Expected 2 elements per point, got {len(p)}")
        flat.extend([float(p[0]), float(p[1])])
except Exception as e:
    plpy.error(f"Invalid polygon shape: {e!s}")

return flat
$probe$;

CREATE FUNCTION pg_temp.unpickle_to_jsonb(raw BYTEA) RETURNS JSONB LANGUAGE plpython3u AS $probe$
import pickle, json

if hasattr(raw, 'tobytes'):
    data_bytes = raw.tobytes()
else:
    data_bytes = raw

try:
    py_obj = pickle.loads(data_bytes)
except Exception as e:
    plpy.error(f"Could not unpickle snapshot data: {e!s}")

return json.dumps(py_obj)
$probe$;

-- A PL/pgSQL wrapper turns each row's failure into a value instead of aborting
-- the probe, so one bad row does not hide the other 499. Neither wrapper is
-- STRICT, matching the real functions: the conversion calls unpickle_to_array
-- on NULL polygons, and dies there.
CREATE FUNCTION pg_temp.try_polygon(raw BYTEA) RETURNS TEXT LANGUAGE plpgsql AS $w$
BEGIN
    PERFORM pg_temp.unpickle_to_array(raw);
    RETURN NULL;
EXCEPTION WHEN OTHERS THEN
    RETURN SQLERRM;
END;
$w$;

CREATE FUNCTION pg_temp.try_snapshot(raw BYTEA) RETURNS TEXT LANGUAGE plpgsql AS $w$
BEGIN
    PERFORM pg_temp.unpickle_to_jsonb(raw);
    RETURN NULL;
EXCEPTION WHEN OTHERS THEN
    RETURN SQLERRM;
END;
$w$;

-- No NULL filter on post_note: the conversion has none either.
SELECT 'post_note.polygon: ' || err || ' (' || count(*) || ' of the sample)'
FROM (SELECT pg_temp.try_polygon(polygon) AS err FROM public.post_note LIMIT 500) s
WHERE err IS NOT NULL GROUP BY err
UNION ALL
SELECT 'snapshot.data: ' || err || ' (' || count(*) || ' of the sample)'
FROM (SELECT pg_temp.try_snapshot(data) AS err FROM public.snapshot WHERE data IS NOT NULL LIMIT 500) s
WHERE err IS NOT NULL GROUP BY err;
ROLLBACK;
SQL
)
    # Capture the status separately. `$(... || true)` would turn any psql
    # failure -- connection loss, a plpython crash, CREATE EXTENSION failing --
    # into empty output, i.e. a reported pass. The one guard here must not
    # fail open.
    #
    # db_query_script, not db_query: this is ten statements, and `psql -c`
    # returns only the last command's result before psql 15. That last command
    # is the ROLLBACK, whose tag -q suppresses -- so on every Debian 11 or
    # Ubuntu 22.04 host (psql 14) the failure list came back empty and this
    # probe reported a clean sample no matter what the sample contained.
    failures="$(db_query_script TGT "$probe_sql" 2>&1)" || status=$?
    if (( status != 0 )); then
        print_error "The unpickle probe could not run (psql exit $status):"
        printf '%s\n' "$failures" >&2
        die "Refusing to convert: the probe that would have caught bad pickled data did not execute."
    fi

    if [[ -n "$failures" ]]; then
        print_error "Server-side unpickling failed on sample rows:"
        printf '%s\n' "$failures" >&2
        print_info "An ImportError names the module to install on the database host (upstream's installer adds SQLAlchemy for this reason)."
        # Deliberately not --force: that flag is what the resume hints hand out
        # after an unrelated step failed, so honouring it here would disarm this
        # guard for an operator who passed it to re-enter `init`.
        if [[ "$FORCE_UNPICKLE" != true ]]; then
            die "Refusing to convert. Fix the above, or re-run with --force-unpickle to proceed anyway."
        fi
        print_warning "--force-unpickle given; converting despite probe failures."
    else
        print_info "Sample rows unpickled cleanly."
    fi
}

step_convert() {
    # The grant must come first. CREATE EXTENSION on an untrusted language is
    # superuser-only, so ensure_plpython3u probing a not-yet-granted oxibooru
    # role reports "unavailable" for a package that is installed. The revoke is
    # armed by the grant itself, so a later failure here still unwinds it.
    grant_superuser_if_needed
    ensure_plpython3u
    if [[ "$DRY_RUN" != true ]]; then
        probe_unpickle
    fi

    local -a flags=()
    if [[ "$SINGLE_TRANSACTION" == true ]]; then
        flags+=(--single-transaction)
        print_info "Single transaction mode: any error rolls the whole conversion back."
    else
        print_warning "Not using a single transaction; an error will leave a partial conversion."
    fi

    print_step convert "Running $CONVERT_SQL"
    # ON_ERROR_STOP is not optional here. Without it, --single-transaction makes
    # psql report the error, abort, turn the final COMMIT into a ROLLBACK -- and
    # exit 0. The conversion would then "succeed", the admin tasks would run
    # against a database still holding the szuru schema, and nothing would say
    # so. (Without --single-transaction it is worse: exit 0, partly committed.)
    # The conversion SQL sets client_min_messages = error, so errors are not
    # expected here and stopping on the first one is right.
    #
    # db_exec_file, not `-f "$CONVERT_SQL"`: in docker mode psql runs inside the
    # container and cannot open a path on this host.
    db_exec_file TGT "$CONVERT_SQL" strict "${flags[@]}" -o /dev/null

    revoke_superuser_if_granted

    print_info "Database conversion complete."
    print_warning "Tags/pools whose names differed only by case were renamed to {name}_name_modified_{id}_{order}."
    print_warning "Search '*_name_modified_*' in the tag/pool search bar to find them."
}

# -----------------------------------------------------------------------------
# Steps: admin CLI tasks
# -----------------------------------------------------------------------------

# The admin tasks run inside the server container, so it (and the database it
# talks to) has to be up: see start_oxi_stack, which is a no-op when everything
# is already running and is therefore safe before every admin step.

step_filenames() {
    start_oxi_stack
    print_step filenames "Running reset_filenames"
    oxi_admin reset_filenames
}

step_thumbsizes() {
    start_oxi_stack
    print_step thumbsizes "Running reset_thumbnail_sizes"
    oxi_admin reset_thumbnail_sizes
}

step_checksums() {
    start_oxi_stack
    print_step checksums "Running recompute_checksums (this takes a while on large databases)"
    oxi_admin recompute_checksums true
}

step_signatures() {
    start_oxi_stack
    print_step signatures "Running recompute_signatures (this takes a while on large databases)"
    oxi_admin recompute_signatures true
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------

main() {
    # A real run wants the strict options and the application_name, whether it
    # got here by execution or from a caller that sourced the file and called
    # main itself.
    set -Eeuo pipefail
    export PGAPPNAME="$APP_NAME"

    install_traps
    print_header "Szurubooru to Oxibooru Conversion"
    parse_args "$@"
    resolve_config

    run_step preflight   step_preflight
    run_step data        step_data
    run_step dump        step_dump
    run_step init        step_init
    run_step restore     step_restore
    run_step convert     step_convert
    run_step filenames   step_filenames
    run_step thumbsizes  step_thumbsizes
    run_step checksums   step_checksums
    run_step signatures  step_signatures

    CURRENT_STEP="cleanup"

    # Bring the stack back up, if we were told enough to do so. The server is ours
    # to start -- it has a spec. The client only gets started when --oxi-client
    # names it; without that, this script has no idea what else your stack contains,
    # and guessing is how you end up starting a database you deliberately removed.
    #
    # A server container this script stopped is restarted whatever steps ran: the
    # gate below used to be `should_run signatures` alone, so `--only convert --force`
    # stopped the instance and left it down without a word.
    #
    # SERVER_WAS_RUNNING is the other half of that. init stops the server it started
    # for the migrations, so `--to init` reaches here with SERVER_STOPPED_BY_SCRIPT
    # set even on a fresh installation whose server has never run -- and starting one
    # that was down before we arrived is not "putting it back".
    if [[ "$OXI_SERVER_MODE" == "docker" ]] && conversion_left_unfinished; then
        # Whatever else is true, do not hand this database to a server. `restore`
        # renames the Oxibooru schema to 'oxi' and puts the Szurubooru dump in
        # 'public'; until `convert` finishes, that is what the server would find
        # -- and it would apply diesel's migrations straight into the Szurubooru
        # schema, on top of the data this run is halfway through migrating.
        print_warning "Not starting $OXI_SERVER_CONTAINER: this run stopped between 'restore' and 'convert', so 'public' still holds the Szurubooru dump and the Oxibooru schema is in 'oxi'."
        print_warning "Starting the server now would have diesel migrate the wrong schema. Finish the conversion first: --from convert --force"
    elif [[ "$OXI_SERVER_MODE" == "docker" ]] \
       && { should_run signatures || [[ "$SERVER_STOPPED_BY_SCRIPT" == true && "$SERVER_WAS_RUNNING" == true ]]; }; then
        start_oxi_stack
        if should_run signatures && [[ -n "$OXI_CLIENT_CONTAINER" ]]; then
            print_step cleanup "Starting the Oxibooru client container"
            run docker start "$OXI_CLIENT_CONTAINER"
        elif should_run signatures; then
            print_info "Server is up. Bring the rest of your stack up as you normally would (--oxi-client starts one more container for you)."
        else
            print_info "Restarted $OXI_SERVER_CONTAINER, which this script had stopped for this run."
        fi
    fi

    # Only tidy up after a run that actually went end to end; a targeted run has no
    # business deleting a dump someone may still need.
    #
    # `should_run dump` is what makes that true. Checking only for --only left every
    # --from resume deleting the dump: `--from checksums` after a died admin step
    # runs through signatures, so cleanup removed a backup.sql this run never
    # produced -- and with --move-data the Szurubooru tree has already been moved
    # away, which makes that dump the only copy of the source database.
    if [[ "$KEEP_DUMP" != true && -f "$DUMP_FILE" && ${#ONLY_STEPS[@]} -eq 0 ]] \
       && should_run dump && should_run signatures; then
        print_step cleanup "Removing $DUMP_FILE"
        run rm -f "$DUMP_FILE"
    fi

    print_header "Conversion Complete"

    print_info "Remaining manual tasks:"
    echo "  - Reset user passwords (admin CLI: reset_passwords); they cannot be migrated"
    echo "  - Port config.yaml settings to config.toml by hand"
    echo "  - Check for tags/pools renamed on case collision (*_name_modified_*)"
    # Upstream's script left the source stack down (it ran `docker compose down`).
    # This one does not touch a stack it was not given, so say the quiet part.
    echo "  - STOP SZURUBOORU if it is still running. This script does not shut it"
    echo "    down, and uploads made to it after the dump will not exist in oxibooru."
    if [[ -n "$SU_DB_SPEC" ]]; then
        # Only the preflight and convert superuser paths populate this, so a run
        # that skipped the convert step used to print an empty role name and a query
        # that matches nothing.
        resolve_tgt_role
        if [[ -n "$TGT_DB_ROLE" ]]; then
            echo "  - Confirm '$TGT_DB_ROLE' no longer has SUPERUSER:"
            echo "      SELECT rolsuper FROM pg_roles WHERE rolname = '$TGT_DB_ROLE';"
        else
            echo "  - Confirm the Oxibooru role no longer has SUPERUSER:"
            echo "      SELECT rolname, rolsuper FROM pg_roles WHERE rolsuper;"
        fi
    fi
    if [[ "$DATA_MODE" == "link" ]]; then
        echo "  - The Szurubooru data tree is still intact (hard links); remove it once you are satisfied"
    fi
}

# Only run a migration when executed; a test harness that sources this file
# gets all 80-odd functions above with nothing happening to its environment.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
