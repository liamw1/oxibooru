# Install Oxibooru

## Prerequisites

This guide assumes that you have Docker (version 19.03 or greater) and the Docker Compose CLI (version 1.27.0 or greater) already installed.

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
    edit server/config.toml
    ```
    It's *strongly recommended* to at least change these fields:

    - password_secret
    - content_secret

    Any fields not present will default to their corresponding value in the original `config.toml.dist`, so feel free to remove fields that are uneeded or irrelevant.

3. **Configure Docker Compose**

    ```sh
    cp example.env .env
    edit .env
    ```

    Change the values of the variables in `.env` as needed. Read the comments to guide you. Note that `.env` should be in the root directory of this repository.

4. **Pull the containers**
    
    This pulls the latest containers from docker.io:

    ```sh
    docker compose pull
    ```

    If you have modified the application's source and would like to manually build it, follow the instructions in [Building](#building) instead, then read here once you're done.

5. **Give mount directories permissions**

    Set owner of mount directories (MOUNT_DATA and MOUNT_SQL in the .env) to the user with id 1000:

    ```sh
    sudo chown -R 1000:1000 "$MOUNT_DATA"
    sudo chown -R 1000:1000 "$MOUNT_SQL"
    ```

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

## Versioning

Oxibooru uses a semantic versioning system to distinguish between backwards compatible and backwards incompatbile versions. An increase in the leading non-zero version number means that the change is breaking, meaning that once you upgrade it may be difficult or impossible to revert to a previous version (typically due to changes in the database schema or data store). For example, it's fine to swap back and forth between versions `0.6.0` and `0.6.2`, but an upgrade from `0.6.2` and `0.7.0` is irreversible. A breaking change may also necesitate downtime due to expensive database migrations.

To change the version of an image, edit the image field in `docker-compose.yml`. The field follows the format `oxibooru/[server|client]:<version>`. There are a few different options for image versioning depending on your appetite for volatility. For maximum stability, you can use the fixed version images, e.g. `0.6.1`. These are never modified after creation, so you can pretty much guarantee that their behavior will stay the same forever. If you like being on the cutting edge, use the `latest` images. They are frequently (but not recklessly) updated with the latest fixes and features, but may introduce breaking changes or time-consuming migrations at any time. For a middle ground, there's also the major and minor version images, e.g. `0` and `0.7`, which are updated with the latest changes up to that major or minor version number.

## Building

To build the client and server containers, run

```sh
docker compose build
```

*Note: If your changes are not taking effect in your builds, consider building
with `--no-cache`.*

#### Performance tip

If you're already building yourself, consider setting `TARGET_CPU` to `native` in the `docker-compose.yml`. This instructions the Rust compiler to target your exact CPU architecture, resulting in better codegen and may result in measurable performance improvements to image decoding and reverse search operations.