error-network = Network error, trying again.
error-process-terminated = Server process was unexpectedly terminated
error-auth-missing = Authorization token is missing
error-measurement = Measurement error
error-finding-free-port = Error finding free port

# Connection states
connecting = Connecting to server...
measuring-speed = Measuring connection speed...

# Progress messages
downloading-webserver = Downloading web server
unpacking-webserver = Unpacking web server
downloading-vcpp = Downloading VC++ components
installing-vcpp = Installing VC++ components

# Progress templates
progress-files = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} files
progress-files-eta = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} files ({"{"}eta{"}"})
progress-bytes = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} bytes ({"{"}eta{"}"})

# Minecraft plugin messages
downloading-jdk = Downloading JDK
installing-jdk = Installing JDK
downloading-minecraft-server = Downloading Minecraft server
error-downloading-jdk = Error downloading JDK
error-unpacking-jdk = Error unpacking JDK
error-copying-minecraft-server = Error copying Minecraft server: {$path}
error-invalid-minecraft-jar-directory = Invalid path to Minecraft server JAR file: {$path} (directory)
error-downloading-minecraft-server = Error downloading Minecraft server: {$url}
error-invalid-minecraft-path = Invalid path or URL to Minecraft server: {$path}
error-creating-server-directory = Error creating server directory
error-creating-server-properties = Error creating server.properties
error-creating-eula-file = Error creating eula.txt file
error-reading-server-properties = Error reading server.properties
error-writing-server-properties = Error writing server.properties
error-getting-java-path = Error getting path to java

# Error contexts
error-downloading-webserver = Error downloading web server
error-unpacking-webserver = Error unpacking web server
error-downloading-vcpp = Error downloading VC++ components
error-installing-vcpp = Error installing VC++ components
error-setting-permissions = Error setting execution permissions
error-creating-marker = Error creating marker file
error-writing-httpd-conf = Error writing httpd.conf

# Service messages
service-published = Service published: {$endpoint}
service-error = Service error: {$endpoint}
service-registered = Service registered: {$endpoint}
service-stopped = Service stopped: {$guid}
service-removed = Service removed: {$guid}
no-registered-services = No registered services
all-services-removed = All services removed

# Authentication
enter-email = Enter email:{" "}
enter-password = Enter password:{" "}
session-terminated = Session terminated, authorization token reset
client-authorized = Client successfully authorized
upgrade-available = New version available: {$version}. Run `clo upgrade` to install.

# Ping statistics
ping-time-percentiles = Ping time (percentiles):

# Invalid formats
invalid-url = Invalid URL
invalid-protocol = Invalid protocol
invalid-address = Invalid address: {$address}
invalid-address-error = Invalid address ({$error}): {$address}
port-required = Port is required for this protocol

# Service messages
service-installed = Service installed successfully
service-uninstalled = Service uninstalled successfully
service-started = Service started successfully
service-stopped-service = Service stopped successfully
service-running = Service is running
service-stopped-status = Service is stopped
service-not-installed = Service is not installed
service-status-unknown = Service status is unknown

# Update messages
applying-update = Applying update and restarting...
downloading-update = Downloading update to {$path}
update-downloaded = Update downloaded to: {$path}
update-unpacked = Update unpacked to: {$path}

# Cache messages
purge-cache-dir = Purge cache dir: {$path}

# GUI messages
show-window = Show CloudPub
quit = Quit
