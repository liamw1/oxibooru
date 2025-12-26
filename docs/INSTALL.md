# Install Oxibooru

## Prerequisites

This guide assumes that you have Docker (version 19.03 or greater) and the Docker Compose CLI (version 1.27.0 or greater) already installed.

## Installing

1. **Download the `oxibooru` source**

    The latest release can be downloaded from the [releases page](https://github.com/liamw1/oxibooru/releases).
    Alternatively, you can clone the repository with

    ```sh
    git clone https://github.com/liamw1/oxibooru
    ```

    However, by doing this you are opting out of stability. The master
    branch uses the `latest` images, which are updated frequently.
    Behavior may change when you `pull` the images again.

    Enter the `oxibooru` directory:

    ```sh
    cd oxibooru
    ```

2. **Configure the application**

    ```sh
    cp server/config.toml.dist server/config.toml
    edit server/config.toml
    ```
    It's strongly recommended to at least change these fields:

    - password_secret
    - content_secret

    Any fields not present will default to their corresponding value
    in the original `config.toml.dist`

3. **Configure Docker Compose**

    ```sh
    cp example.env .env
    edit .env
    ```

    Change the values of the variables in `.env` as needed.
    Read the comments to guide you. Note that `.env` should be in the root
    directory of this repository.

4. **Pull the containers**
    This pulls the latest containers from docker.io:

    ```sh
    docker compose pull
    ```

    If you have modified the application's source and would like to manually
    build it, follow the instructions in [Building](#building) instead,
    then read here once you're done.

5. **Give mount directories permissions**

    Set owner of mount directories (MOUNT_DATA and MOUNT_SQL in the .env) to the user with id 1000:

    ```sh
    sudo chown -R 1000:1000 <MOUNT_DATA>
    sudo chown -R 1000:1000 <MOUNT_SQL>
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

## Building

To build the client and server containers, run

```sh
docker compose build
```

*Note: If your changes are not taking effect in your builds, consider building
with `--no-cache`.*
