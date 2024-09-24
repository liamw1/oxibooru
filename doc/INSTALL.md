This assumes that you have Docker (version 17.05 or greater)
and Docker Compose (version 1.6.0 or greater) already installed.

### Prepare things

1. Install dependencies

    Diesel cli:
    ```console
    user@host:~$ curl --proto '=https' --tlsv1.2 -LsSf https://github.com/diesel-rs/diesel/releases/latest/download/diesel_cli-installer.sh | sh
    ```
    Restart your terminal.

2. Download the `oxibooru` source
    ```console
    user@host:~$ git clone https://github.com/liamw1/oxibooru
    user@host:~$ cd oxibooru
    ```

3. Configure the application
    ```console
    user@host:oxibooru$ cp server/config.toml.dist server/config.toml
    user@host:oxibooru$ edit server/config.toml
    ```
    Pay extra attention to these fields:

    - password_secret
    - content_secret

4. Configure Docker Compose
    ```console
    user@host:oxibooru$ cp doc/example.env .env
    user@host:oxibooru$ edit .env
    ```
    Change the values of the variables in `.env` as needed.
    Read the comments to guide you. Note that `.env` should be in the root
    directory of this repository.

5. Give mount directories permissions

    Set owner of mount directories (MOUNT_DATA and MOUNT_SQL in the .env) to the user with id 1000:
    ```console
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_DATA>
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_SQL>
    ```

6. Setup database

    First, start database container:
    ```console
    user@host:oxibooru$ docker-compose up -d sql
    ```
    Then, run migration scripts:
    ```console
    user@host:oxibooru$ cd server
    user@host:oxibooru/server$ diesel migration run --database-url=postgres://<POSTGRES_USER>:<POSTGRES_PASSWORD>@localhost:<POSTGRES_PORT>/<POSTGRES_DB>
    ```

7. Build the containers:
    ```console
    user@host:oxibooru$ docker-compose build
    ```

8. Run it!

    To start all containers:
    ```console
    user@host:oxibooru$ docker-compose up -d
    ```
    To view/monitor the application logs:
    ```console
    user@host:oxibooru$ docker-compose logs -f
    # (CTRL+C to exit)
    ```