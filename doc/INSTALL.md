This assumes that you have Docker (version 19.03 or greater)
and the Docker Compose CLI (version 1.27.0 or greater) already installed.

## Prepare things

1. **Download the `oxibooru` source**
    
    The latest release can be downloaded from the [releases page](https://github.com/liamw1/oxibooru/releases).
    Alternatively, you can clone the repository with
    ```console
    user@host:~$ git clone https://github.com/liamw1/oxibooru
    ```
    However, by doing this you are opting out of stability. The master
    branch uses the `latest` images, which are updated frequently.
    Behavior may change when you `pull` the images again.

    Enter the `oxibooru` directory:
    ```console
    user@host:~$ cd oxibooru
    ```

2. **Configure the application**
    ```console
    user@host:oxibooru$ cp server/config.toml.dist server/config.toml
    user@host:oxibooru$ edit server/config.toml
    ```
    Pay extra attention to these fields:

    - password_secret
    - content_secret

3. **Configure Docker Compose**
    ```console
    user@host:oxibooru$ cp doc/example.env .env
    user@host:oxibooru$ edit .env
    ```
    Change the values of the variables in `.env` as needed.
    Read the comments to guide you. Note that `.env` should be in the root
    directory of this repository.

4. **Pull the containers**
    This pulls the latest containers from docker.io:
    ```console
    user@host:oxibooru$ docker compose pull
    ```
    If you have modified the application's source and would like to manually
    build it, follow the instructions in [**Building**](#Building) instead,
    then read here once you're done.

5. **Give mount directories permissions**

    Set owner of mount directories (MOUNT_DATA and MOUNT_SQL in the .env) to the user with id 1000:
    ```console
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_DATA>
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_SQL>
    ```

6. **Run it!**

    To start all containers:
    ```console
    user@host:oxibooru$ docker compose up -d
    ```
    To view/monitor the application logs:
    ```console
    user@host:oxibooru$ docker compose logs -f
    # (CTRL+C to exit)
    ```

## Building

1. **Build the containers**

    First, start database container:
    ```console
    user@host:oxibooru$ docker compose up -d sql
    ```
    Then, build the client and server containers:
    ```console
    user@host:oxibooru$ docker compose build
    ```

*Note: If your changes are not taking effect in your builds, consider building
with `--no-cache`.*
