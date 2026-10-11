# Install Oxibooru

## Prerequisites

This guide assumes that you have Docker and the Docker Compose plugin (`docker compose`) already installed.

## Installing

1. **Download the `oxibooru` source**

    To get started either clone the repository with

    ```sh
    git clone https://github.com/liamw1/oxibooru
    ```

    or download the latest release from the [releases page](https://github.com/liamw1/oxibooru/releases). By default, a freshly cloned repo will use the `latest` images, whereas a release package will be fixed to a particular version. **Be warned that by using `latest` images, you are opting out of stability**. See [Versioning](#versioning) for details.

    Enter the `oxibooru` directory:

    ```sh
    cd oxibooru
    ```

2. **Configure the application**

    ```sh
    cp server/config.toml.dist server/config.toml
    ```

    Open the `config.toml` in an editor of your choice to change the server configuration. It's *strongly recommended* to at least change these fields:

    - password_secret
    - content_secret

    These fields cannot be easily changed once the database is initialized.

    Any fields not present will default to their corresponding value in the original `config.toml.dist`, so feel free to remove fields that are unneeded or irrelevant.

3. **Configure Docker Compose**

    ```sh
    cp example.env .env
    ```

    Open `.env` in an editor of your choice and change the values of the variables as needed. Read the comments to guide you. Note that `.env` should be in the root directory of this repository.

4. **Pull the containers**
    
    This pulls the latest containers from docker.io:

    ```sh
    docker compose pull
    ```

    If you have modified the application's source and would like to manually build it, follow the instructions in [Building](#building) instead, then continue with step 5 when you are done.

5. **Give permissions**

    Create your data directory (the one specified by `MOUNT_DATA` in the `.env`) if it does not exist and set the owner to uid 1000:

    ```sh
    sudo mkdir -p <path-to-data-dir>
    sudo chown -R 1000:1000 <path-to-data-dir>
    ```

    **Note:** The `config.toml` must be readable by uid 1000 inside the container, which is usually the case for typical setups. If the server reports `Permission denied` on startup, run `chmod o+r server/config.toml`.

6. **Run it!**

    To start all containers:

    ```sh
    docker compose up -d
    ```

    To view/monitor the application logs:

    ```sh
    docker compose logs -f
    # (CTRL+C to exit)
    ```

    There are no default credentials for the installation. Instead, the first user registered via the UI will have `administrator` rank. All subsequent users will have rank specified by `default_user_rank` in the `config.toml`.

## Updating

Before updating, check the [releases page](https://github.com/liamw1/oxibooru/releases) for breaking changes. If the update is breaking (see [Versioning](#versioning)), back up your data and SQL directories first, since you won't be able to go back.

1. **Get the new version**

    If you cloned the repository:

    ```sh
    git pull
    ```

    If you are using a fixed version, change the version in the `image` fields of `docker-compose.yml` instead.

2. **Get the new containers**

    ```sh
    docker compose pull
    ```

    If you build the containers yourself, run `docker compose build` instead.

3. **Recreate the containers**

    ```sh
    docker compose up -d
    ```

    Pulling or building alone does not update a running instance; the containers must be recreated from the new images. Database migrations are applied automatically when the server starts.

4. **Check that the update took effect**

    The build date in the footer of the home page should match when the client image was built.

## Versioning

Oxibooru uses a semantic versioning system to distinguish between backwards compatible and backwards incompatible versions. An increase in the leading non-zero version number means that the change is breaking, meaning that once you upgrade it may be difficult or impossible to revert to a previous version (typically due to changes in the database schema or data store). For example, it's fine to swap back and forth between versions `0.6.0` and `0.6.2`, but an upgrade from `0.6.2` to `0.7.0` is irreversible. A breaking change may also necessitate downtime due to expensive database migrations.

To change the version of an image, edit the image field in `docker-compose.yml`. The field follows the format `oxibooru/[server|client]:<version>`. There are a few different options for image versioning depending on your appetite for volatility. For maximum stability, you can use the fixed version images, e.g. `0.6.1`. These are never modified after creation, so you can pretty much guarantee that their behavior will stay the same forever. If you like being on the cutting edge, use the `latest` images. They are frequently (but not recklessly) updated with the latest fixes and features, but may introduce breaking changes or time-consuming migrations at any time. For a middle ground, there's also the major and minor version images, e.g. `0` and `0.7`, which are updated with the latest changes up to that major or minor version number.

## Building

To build the client and server containers, run

```sh
docker compose build
```

**Note:** If your changes are not taking effect in your builds, try recreating the containers:

```sh
docker compose down
docker compose up -d
```

or rebuilding with `docker compose build --no-cache`.

#### Performance tip

If you're already building yourself, consider setting `TARGET_CPU` to `native` in the `docker-compose.yml`. This instructs the Rust compiler to target your exact CPU architecture, resulting in better codegen and may result in measurable performance improvements to image decoding and reverse search operations.