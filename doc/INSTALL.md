This assumes that you have Docker (version 17.05 or greater)
and Docker Compose (version 1.6.0 or greater) already installed.

### Prepare things

1. Download the `oxibooru` source:

    ```console
    user@host:~$ git clone https://github.com/liamw1/oxibooru
    user@host:~$ cd oxibooru
    ```
2. Configure the application:

    ```console
    user@host:szuru$ cp server/config.toml.dist server/config.toml
    user@host:szuru$ edit server/config.toml
    ```

    Pay extra attention to these fields:

    - password_secret
    - content_secret

3. Configure Docker Compose:

    ```console
    user@host:szuru$ cp doc/example.env .env
    user@host:szuru$ edit .env
    ```

    Change the values of the variables in `.env` as needed.
    Read the comments to guide you. Note that `.env` should be in the root
    directory of this repository.

4. Build the containers:

    ```console
    user@host:szuru$ docker-compose build
    ```

5. Run it!

    For first run, it is recommended to start the database separately:
    ```console
    user@host:szuru$ docker-compose up -d sql
    ```

    To start all containers:
    ```console
    user@host:szuru$ docker-compose up -d
    ```

    To view/monitor the application logs:
    ```console
    user@host:szuru$ docker-compose logs -f
    # (CTRL+C to exit)
    ```

### Additional Features

1. **CLI-level administrative tools**

    You can use the included `szuru-admin` script to perform various
    administrative tasks such as changing or resetting a user password. To
    run from docker:

    ```console
    user@host:szuru$ docker-compose run server ./szuru-admin --help
    ```

    will give you a breakdown on all available commands.

2. **Using a seperate domain to host static files (image content)**

    If you want to host your website on, (`http://example.com/`) but want
    to serve the images on a different domain, (`http://static.example.com/`)
    then you can run the backend container with an additional environment
    variable `DATA_URL=http://static.example.com/`. Make sure that this
    additional host has access contents to the `/data` volume mounted in the
    backend.