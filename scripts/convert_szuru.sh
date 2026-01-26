#!/bin/bash

# =============================================================================
# Szurubooru to Oxibooru Conversion Script
# =============================================================================
# This script automates the conversion process from Szurubooru to Oxibooru.
#
# Known Limitations:
# - Passwords cannot be migrated (must be reset manually)
# - HEIF/HEIC file formats are not supported
# - YouTube posts are not supported
# =============================================================================

set -e  # Exit on any error

# -----------------------------------------------------------------------------
# Signal Handling and Cleanup
# -----------------------------------------------------------------------------

cleanup() {
    # Disable the trap to prevent recursive calls
    trap - SIGINT SIGTERM
    
    echo ""
    print_warning "Caught interrupt signal, cleaning up..."
    
    # Kill any docker exec processes we spawned
    pkill -P $$ docker 2>/dev/null || true
    
    # Also stop any running admin CLI in the container
    docker exec "$OXI_SERVER_CONTAINER" pkill -f "./server --admin" 2>/dev/null || true
    
    print_info "Cleanup complete. Exiting."
    exit 130
}

# Trap SIGINT (Ctrl+C) and SIGTERM
trap cleanup SIGINT SIGTERM

# Helper functions to run docker exec commands interruptibly
run_admin_command() {
    local command="$1"
    printf '%s\nexit\n' "$command" | docker exec -i "$OXI_SERVER_CONTAINER" ./server --admin
}
run_admin_post_command() {
    local command="$1"
    printf '%s\n\nexit\n' "$command" | docker exec -i "$OXI_SERVER_CONTAINER" ./server --admin
}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------

print_header() {
    echo -e "\n${BLUE}=============================================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=============================================================================${NC}\n"
}

print_step() {
    echo -e "${GREEN}[STEP $1]${NC} $2"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

confirm() {
    read -p "$1 (y/n): " -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        return 1
    fi
    return 0
}

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------

# Default values - can be overridden via command line or environment
SZURU_DIR="${SZURU_DIR:-}"
OXI_DIR="${OXI_DIR:-}"
SZURU_SQL_CONTAINER="${SZURU_SQL_CONTAINER:-szuru-sql-1}"
OXI_SQL_CONTAINER="${OXI_SQL_CONTAINER:-oxibooru-sql-1}"
OXI_SERVER_CONTAINER="${OXI_SERVER_CONTAINER:-oxibooru-server-1}"
SINGLE_TRANSACTION="${SINGLE_TRANSACTION:-true}"
MOVE_DATA="${MOVE_DATA:-false}"

# These will be read from .env files
SZURU_POSTGRES_USER=""
SZURU_MOUNT_DATA=""
OXI_POSTGRES_USER=""
OXI_POSTGRES_DB=""
OXI_MOUNT_DATA=""

# -----------------------------------------------------------------------------
# Helper Function: Read value from .env file
# -----------------------------------------------------------------------------

# Reads a variable value from a .env file
# Usage: read_env_var <env_file> <variable_name>
# Returns: The value of the variable, or empty string if not found
read_env_var() {
    local env_file="$1"
    local var_name="$2"
    
    if [[ ! -f "$env_file" ]]; then
        return 1
    fi
    
    # Parse the .env file, handling quotes and comments
    local value
    value=$(grep -E "^${var_name}=" "$env_file" 2>/dev/null | head -n1 | cut -d'=' -f2- | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//" -e 's/#.*//' | xargs)
    echo "$value"
}

# -----------------------------------------------------------------------------
# Parse Command Line Arguments
# -----------------------------------------------------------------------------

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Converts a Szurubooru database to Oxibooru format.

Required Options:
    --szuru-dir PATH                Path to Szurubooru source directory
    --oxi-dir PATH                  Path to Oxibooru source directory

Optional Options:
    --szuru-container NAME          Szurubooru SQL container name (default: szuru-sql-1)
    --oxi-sql-container NAME        Oxibooru SQL container name (default: oxibooru-sql-1)
    --oxi-server-container NAME     Oxibooru server container name (default: oxibooru-server-1)
    --move-data                     Move the data directory instead of copying it (faster, but destructive)
    --no-single-transaction         Don't use single transaction for conversion (allows partial conversion)
    -h, --help                      Show this help message

Database credentials (POSTGRES_USER, POSTGRES_DB, MOUNT_DATA) are automatically
read from the .env files in the Szurubooru and Oxibooru directories.

Example:
    $0 --szuru-dir /path/to/szurubooru --oxi-dir /path/to/oxibooru

EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --szuru-dir)
            SZURU_DIR="$2"
            shift 2 ;;
        --oxi-dir)
            OXI_DIR="$2"
            shift 2 ;;
        --szuru-container)
            SZURU_SQL_CONTAINER="$2"
            shift 2 ;;
        --oxi-sql-container)
            OXI_SQL_CONTAINER="$2"
            shift 2 ;;
        --oxi-server-container)
            OXI_SERVER_CONTAINER="$2"
            shift 2 ;;
        --move-data)
            MOVE_DATA="true"
            shift ;;
        --no-single-transaction)
            SINGLE_TRANSACTION="false"
            shift ;;
        -h|--help)
            usage ;;
        *)
            print_error "Unknown option: $1"
            usage ;;
    esac
done

# -----------------------------------------------------------------------------
# Validation
# -----------------------------------------------------------------------------

print_header "Szurubooru to Oxibooru Conversion Script"

# Check required parameters
missing_params=()
[[ -z "$SZURU_DIR" ]] && missing_params+=("--szuru-dir")
[[ -z "$OXI_DIR" ]] && missing_params+=("--oxi-dir")

if [[ ${#missing_params[@]} -gt 0 ]]; then
    print_error "Missing required parameters: ${missing_params[*]}"
    echo ""
    usage
fi

# Validate directories exist
if [[ ! -d "$SZURU_DIR" ]]; then
    print_error "Szurubooru directory does not exist: $SZURU_DIR"
    exit 1
fi

if [[ ! -d "$OXI_DIR" ]]; then
    print_error "Oxibooru directory does not exist: $OXI_DIR"
    exit 1
fi

# Check for docker
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed or not in PATH"
    exit 1
fi

# -----------------------------------------------------------------------------
# Read Configuration from .env Files
# -----------------------------------------------------------------------------

print_info "Reading configuration from .env files..."

# Read Szurubooru .env
SZURU_ENV_FILE="$SZURU_DIR/.env"
if [[ ! -f "$SZURU_ENV_FILE" ]]; then
    print_error "Szurubooru .env file not found: $SZURU_ENV_FILE"
    exit 1
fi

SZURU_POSTGRES_USER=$(read_env_var "$SZURU_ENV_FILE" "POSTGRES_USER")
if [[ -z "$SZURU_POSTGRES_USER" ]]; then
    print_error "POSTGRES_USER not found in $SZURU_ENV_FILE"
    exit 1
fi

SZURU_MOUNT_DATA=$(read_env_var "$SZURU_ENV_FILE" "MOUNT_DATA")
if [[ -z "$SZURU_MOUNT_DATA" ]]; then
    print_error "MOUNT_DATA not found in $SZURU_ENV_FILE"
    exit 1
fi

# Read Oxibooru .env
OXI_ENV_FILE="$OXI_DIR/.env"
if [[ ! -f "$OXI_ENV_FILE" ]]; then
    print_error "Oxibooru .env file not found: $OXI_ENV_FILE"
    exit 1
fi

OXI_POSTGRES_USER=$(read_env_var "$OXI_ENV_FILE" "POSTGRES_USER")
if [[ -z "$OXI_POSTGRES_USER" ]]; then
    print_error "POSTGRES_USER not found in $OXI_ENV_FILE"
    exit 1
fi

OXI_POSTGRES_DB=$(read_env_var "$OXI_ENV_FILE" "POSTGRES_DB")
if [[ -z "$OXI_POSTGRES_DB" ]]; then
    print_error "POSTGRES_DB not found in $OXI_ENV_FILE"
    exit 1
fi

OXI_MOUNT_DATA=$(read_env_var "$OXI_ENV_FILE" "MOUNT_DATA")
if [[ -z "$OXI_MOUNT_DATA" ]]; then
    print_error "MOUNT_DATA not found in $OXI_ENV_FILE"
    exit 1
fi

# -----------------------------------------------------------------------------
# Display Configuration
# -----------------------------------------------------------------------------

print_info "Configuration:"
echo "  Szurubooru directory:       $SZURU_DIR"
echo "  Oxibooru directory:         $OXI_DIR"
echo "  Szurubooru POSTGRES_USER:   $SZURU_POSTGRES_USER"
echo "  Szurubooru MOUNT_DATA:      $SZURU_MOUNT_DATA"
echo "  Oxibooru POSTGRES_USER:     $OXI_POSTGRES_USER"
echo "  Oxibooru POSTGRES_DB:       $OXI_POSTGRES_DB"
echo "  Oxibooru MOUNT_DATA:        $OXI_MOUNT_DATA"
echo "  Szurubooru SQL container:   $SZURU_SQL_CONTAINER"
echo "  Oxibooru SQL container:     $OXI_SQL_CONTAINER"
echo "  Oxibooru server container:  $OXI_SERVER_CONTAINER"
echo "  Move data (not copy):       $MOVE_DATA"
echo "  Single transaction:         $SINGLE_TRANSACTION"
echo ""

# -----------------------------------------------------------------------------
# Step 1: Pre-flight Checks and Data Directory Copy
# -----------------------------------------------------------------------------

print_header "Step 1: Pre-flight Checks and Data Directory Setup"

# Verify Szurubooru data directory exists
if [[ ! -d "$SZURU_MOUNT_DATA" ]]; then
    print_error "Szurubooru data directory does not exist: $SZURU_MOUNT_DATA"
    exit 1
fi

# Verify Oxibooru data directory does NOT exist yet
if [[ -d "$OXI_MOUNT_DATA" ]]; then
    print_error "Oxibooru data directory already exists: $OXI_MOUNT_DATA"
    print_error "This script expects a fresh Oxibooru installation without existing data."
    print_info "Please remove the data directory or use a fresh Oxibooru clone."
    exit 1
fi

if [[ "$MOVE_DATA" == "true" ]]; then
    print_step 1 "Moving Szurubooru data directory to Oxibooru..."
else
    print_step 1 "Copying Szurubooru data directory to Oxibooru..."
fi
print_info "Source: $SZURU_MOUNT_DATA"
print_info "Destination: $OXI_MOUNT_DATA"

# Create parent directory if it doesn't exist
OXI_MOUNT_DATA_PARENT=$(dirname "$OXI_MOUNT_DATA")
if [[ ! -d "$OXI_MOUNT_DATA_PARENT" ]]; then
    print_step 1 "Creating parent directory: $OXI_MOUNT_DATA_PARENT"
    mkdir -p "$OXI_MOUNT_DATA_PARENT"
fi

if [[ "$MOVE_DATA" == "true" ]]; then
    mv "$SZURU_MOUNT_DATA" "$OXI_MOUNT_DATA"
    print_info "Data directory moved successfully"
else
    cp -r "$SZURU_MOUNT_DATA" "$OXI_MOUNT_DATA"
    print_info "Data directory copied successfully"
fi

# Move custom-thumbnails if they exist
CUSTOM_THUMBS_SRC="$OXI_MOUNT_DATA/posts/custom-thumbnails"
CUSTOM_THUMBS_DST="$OXI_MOUNT_DATA/custom-thumbnails"

if [[ -d "$CUSTOM_THUMBS_SRC" ]]; then
    print_step 1 "Moving custom thumbnails to new location..."
    mv "$CUSTOM_THUMBS_SRC" "$CUSTOM_THUMBS_DST"
    print_info "Custom thumbnails moved to $CUSTOM_THUMBS_DST"
fi

# -----------------------------------------------------------------------------
# Step 2: Create Szurubooru Database Dump
# -----------------------------------------------------------------------------

print_header "Step 2: Creating Szurubooru Database Dump"

cd "$SZURU_DIR"

print_step 2 "Starting Szurubooru SQL container..."
docker compose down
docker compose up -d --wait sql

print_step 2 "Creating database dump..."
docker exec "$SZURU_SQL_CONTAINER" pg_dump -U "$SZURU_POSTGRES_USER" --no-owner --no-privileges szuru > backup.sql

if [[ ! -f "backup.sql" ]] || [[ ! -s "backup.sql" ]]; then
    print_error "Failed to create database dump or dump is empty"
    exit 1
fi

print_info "Database dump created: $SZURU_DIR/backup.sql ($(du -h backup.sql | cut -f1))"

# -----------------------------------------------------------------------------
# Step 3: Initialize Oxibooru and Restore Database
# -----------------------------------------------------------------------------

print_header "Step 3: Initializing Oxibooru and Restoring Database"

cd "$OXI_DIR"

print_step 3 "Starting Oxibooru SQL container (this will create the database)..."
docker compose down
docker compose up -d --wait sql

print_step 3 "Starting Oxibooru server to apply migrations..."
docker compose up -d --wait server

print_step 3 "Applying schema migrations..."
docker compose stop server

print_step 3 "Renaming existing schema..."
docker exec "$OXI_SQL_CONTAINER" psql -U "$OXI_POSTGRES_USER" -d "$OXI_POSTGRES_DB" \
    -c "ALTER SCHEMA public RENAME TO $OXI_POSTGRES_DB;"

print_step 3 "Creating new public schema..."
docker exec "$OXI_SQL_CONTAINER" psql -U "$OXI_POSTGRES_USER" -d "$OXI_POSTGRES_DB" \
    -c "CREATE SCHEMA public;"

print_step 3 "Restoring Szurubooru database..."
cat "$SZURU_DIR/backup.sql" | docker exec -i "$OXI_SQL_CONTAINER" psql -U "$OXI_POSTGRES_USER" -d "$OXI_POSTGRES_DB" -o /dev/null

print_info "Database restored successfully"

# -----------------------------------------------------------------------------
# Step 4: Run Conversion Script
# -----------------------------------------------------------------------------

print_header "Step 4: Running Conversion Script"

print_step 4 "Installing PL/Python procedural language..."
if [[ -f "scripts/install_plpython3u.sh" ]]; then
    docker exec -i "$OXI_SQL_CONTAINER" bash -s < scripts/install_plpython3u.sh
else
    print_error "scripts/install_plpython3u.sh not found in $OXI_DIR"
    exit 1
fi

print_step 4 "Running database conversion script..."
if [[ ! -f "scripts/convert_szuru_database.sql" ]]; then
    print_error "scripts/convert_szuru_database.sql not found in $OXI_DIR"
    exit 1
fi

TRANSACTION_FLAG=""
if [[ "$SINGLE_TRANSACTION" == "true" ]]; then
    TRANSACTION_FLAG="--single-transaction"
    print_info "Using single transaction mode (errors will rollback all changes)"
else
    print_warning "Not using single transaction mode (partial conversion may occur on errors)"
fi

cat scripts/convert_szuru_database.sql | docker exec -i "$OXI_SQL_CONTAINER" psql -U "$OXI_POSTGRES_USER" -d "$OXI_POSTGRES_DB" $TRANSACTION_FLAG -o /dev/null

print_info "Database conversion completed"
print_warning "Note: If you had tags/pools with names differing only by case, they have been"
print_warning "renamed to {name}_name_modified_{tag_id}_{order}. Search for '*_name_modified_*'"
print_warning "in the tag/pool search bar to find affected items."

# -----------------------------------------------------------------------------
# Step 5: Start Oxibooru and Run Admin Tasks
# -----------------------------------------------------------------------------

print_header "Step 5: Starting Oxibooru and Running Admin Tasks"

print_step 5 "Starting all Oxibooru containers..."
docker compose up -d --wait

# Create an expect-like script to interact with the admin CLI
print_step 5 "Running reset_filenames..."

run_admin_command "reset_filenames"

print_info "Filenames reset completed"

# -----------------------------------------------------------------------------
# Step 6: Compute Thumbnail Sizes
# -----------------------------------------------------------------------------

print_header "Step 6: Computing Thumbnail Sizes"

print_step 6 "Running reset_thumbnail_sizes..."
run_admin_command "reset_thumbnail_sizes"

print_info "Thumbnail sizes computed"

# -----------------------------------------------------------------------------
# Step 7: Recompute Post Properties
# -----------------------------------------------------------------------------

print_header "Step 7: Recomputing Post Properties"

print_step 7 "Recomputing post checksums (this may take a while for large databases)..."

run_admin_post_command "recompute_checksums"

print_info "Checksums recomputed"

print_step 7 "Recomputing post signatures (this may take a while for large databases)..."

run_admin_post_command "recompute_signatures"

print_info "Signatures recomputed"

# -----------------------------------------------------------------------------
# Cleanup
# -----------------------------------------------------------------------------

print_header "Cleanup"

if [[ -f "$SZURU_DIR/backup.sql" ]]; then
    print_step 8 "Deleting backup.sql..."
    rm "$SZURU_DIR/backup.sql"
    print_info "Backup file deleted"
fi

# -----------------------------------------------------------------------------
# Complete
# -----------------------------------------------------------------------------

print_header "Conversion Complete!"

echo -e "${GREEN}The Szurubooru to Oxibooru conversion has been completed successfully!${NC}"
echo ""
print_info "Summary of what was done:"
echo "  ✓ Data directory copied from Szurubooru to Oxibooru"
echo "  ✓ Custom thumbnails relocated (if present)"
echo "  ✓ Database dumped from Szurubooru"
echo "  ✓ Oxibooru initialized with fresh database"
echo "  ✓ Szurubooru database restored and converted"
echo "  ✓ Filenames updated to Oxibooru format"
echo "  ✓ Thumbnail sizes computed"
echo "  ✓ Post checksums recomputed"
echo "  ✓ Post signatures recomputed"
echo ""
print_warning "Remaining manual tasks:"
echo "  • Reset user passwords as needed"
echo "  • Migrate config.yaml settings to config.toml manually"
echo "  • Check for tags/pools renamed due to case conflicts (*_name_modified_*)"
echo ""
print_info "Your Oxibooru instance should now be accessible!"