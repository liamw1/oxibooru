This assumes that you have Docker (version 17.05 or greater)
and Docker Compose (version 1.6.0 or greater) already installed.

### Prepare things

1. Download the `oxibooru` source
    ```console
    user@host:~$ git clone https://github.com/liamw1/oxibooru
    user@host:~$ cd oxibooru
    ```

2. Configure the application
    ```console
    user@host:oxibooru$ cp server/config.toml.dist server/config.toml
    user@host:oxibooru$ edit server/config.toml
    ```
    Pay extra attention to these fields:

    - password_secret
    - content_secret

3. Configure Docker Compose
    ```console
    user@host:oxibooru$ cp doc/example.env .env
    user@host:oxibooru$ edit .env
    ```
    Change the values of the variables in `.env` as needed.
    Read the comments to guide you. Note that `.env` should be in the root
    directory of this repository.

4. Give mount directories permissions

    Set owner of mount directories (MOUNT_DATA and MOUNT_SQL in the .env) to the user with id 1000:
    ```console
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_DATA>
    user@host:oxibooru$ sudo chown -R 1000:1000 <MOUNT_SQL>
    ```

5. Build the containers:

    First, start database container:
    ```console
    user@host:oxibooru$ docker-compose up -d sql
    ```
    Then, build the client and server containers:
    ```console
    user@host:oxibooru$ docker-compose build
    ```

6. Run it!

    To start all containers:
    ```console
    user@host:oxibooru$ docker-compose up -d
    ```
    To view/monitor the application logs:
    ```console
    user@host:oxibooru$ docker-compose logs -f
    # (CTRL+C to exit)
    ```