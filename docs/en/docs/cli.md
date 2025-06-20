---
sidebar_position: 99
slug: /cli
---

# Command Line Parameters

clo provides the following command line parameters:

```bash
Usage: clo [Options] <Command>

Commands:
  login      Authenticate on server
  logout     End session
  publish    Publish resource
  register   Publish resource without starting application
  unpublish  Unpublish resource
  ls         List published resources
  clean      Remove all published resources
  run        Start all published resources
  purge      Clear cache (downloads and installed third-party applications)
  set        Set configuration parameter value
  get        Get configuration parameter value
  ping       Check ping to server
  service    Work with service
  upgrade    Check and upgrade to the latest version
  help       Help

Options:
  -v, --verbose                Output log to console
  -l, --log-level <LOG_LEVEL>  Logging level, default: "error".
                               Possible values: "error", "warn", "info", "debug"
  -c, --conf <CONF>            Path to configuration file
  -h, --help                   Show help
  -V, --version                Show version number
```

## Command Descriptions

### Authenticate on Server

```bash
clo login <email> [--password password]
```

Authenticates on the server using email and password, and saves the API token to the configuration file.

If password is not specified as a command line argument, the command will prompt for it or use the `CLO_PASSWORD` environment variable if it's set.

### Publish Resource

```bash
clo publish [--name service_name] [--auth auth_type] [--acl email:role] [--header name:value] <protocol> <port|host:port|path|connection_string>
```

#### Parameters

- `protocol` - protocol by which the resource is accessible (http, https, tcp, udp, 1c)
- `port` - port number on which the resource is accessible
- `host` - host address on which the resource is accessible. If host is not specified, `localhost` is used
- `path` - for 1c protocol - path to 1C database directory
- `connection_string` - for 1c protocol - connection string to 1C database

#### Options

- `--name` - optional service name for display in personal account
- `--auth` - authentication type for resource access. Possible values:
  - `none` - no authentication
  - `basic` - HTTP Basic Auth
- `--acl` - Resource access rule. There can be multiple rules. Each rule has the format `email:role`, where:
  - `email` - user's email address
  - `role` - user role. Possible values:
    - `admin` - administrator
    - `reader` - user with read permissions
    - `writer` - user with write permissions (only for `webdav` protocol)
- `--header` - HTTP header for request. There can be multiple headers. Each rule has the format `name:value`, where:
  - `name` - header name
  - `value` - header value

After publication, the resource is added to the configuration file and the application starts.

If you just need to add a resource to the configuration file, use the `register` command

After that, you can start the application with the `run` command

### Unpublish Resource

```bash
clo unpublish [--remove] <service guid>
```

Unpublishes the resource.

If the `--remove` flag is specified, the resource is removed from the configuration file

### Get List of Published Resources

```bash
clo ls
```

### Set Configuration Value

```bash
clo set <key> <value>
```

The key value can be one of the following:

| Value | Description | Default Value|
| --- | --- | --- |
|`token`|API access token from personal account|None|
|`server`|CloudPub server URL|`https://cloudpub.online`|
|`1c_platform`|1C platform architecture (x64/x86)|`x64`|
|`1c_home`|Path to folder where 1C is installed|Windows:<br/>`C:\Program Files\1cv8`<br/>Linux:<br/>`/opt/1C`|
|`1c_publish_dir`|Path to directory with 1C publication files (`default.vrd`)|Windows:<br/>`%APPDATA%/cloudpub/1c`<br/>Linux: `~/.cache/cloudpub/1c`|
|`minecraft_server`|URL for downloading Minecraft server or local path to jar|[`server.jar`](https://piston-data.mojang.com/v1/objects/45810d238246d90e811d896f87b14695b7fb6839/server.jar)|
|`minecraft_java_opts`|Java options for Minecraft server|`-Xmx2048M -Xms2048M`|
|`usafe_tls`|Ignore server certificate verification|`false`|

### Get Configuration Value

```bash
clo get <key>
```

The key value is the same as for the `set` command

### Start All Previously Saved Resources

```bash
clo run
```

### Check Ping to Server

```bash
clo ping
```

### Service Installation and Management

You can install the application as a service so it automatically starts on system boot and runs in the background.

:::info
Commands for installing and managing the service also require superuser privileges, so you may need to use `sudo` before the commands.

On Windows, commands for installing and managing the service require administrator privileges, so you may need to run the console as administrator.

On Linux, the service will run under the `root` user, so you should configure the API key and other configuration parameters under this user as well.
:::

#### Service Installation

```bash
clo service install
```
Installs the application as a service

#### Start Service

```bash
clo service start
```
Starts the application as a service

#### Stop Service

```bash
clo service stop
```
Stops the application as a service

#### Service Status

```bash
clo service status
```
Shows service status

#### Uninstall Service

```bash
clo service uninstall
```
Removes the application as a service

### Clear Cache

```bash
clo purge
```

Removes all temporary files created during operation

### End Session

```bash
clo logout
```

Removes the saved API token from the configuration file.
