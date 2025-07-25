---
sidebar_position: 1
slug: /
---

import { Downloads, getUrl, getFile } from 'src/components/app/downloads';
import CodeBlock from '@theme/CodeBlock';

# Quick Start

How to start using **CloudPub in 5 minutes**.

## Install the Client

<Downloads />

## Graphical Interface Application

 - Create an account in the [personal account](https://cloudpub.online/dashboard)
 - Launch the application (executable file is called `cloudpub`)
 - Authenticate:

![Authentication](/img/login-form.png)

 - Click the `New Publication` button
 - Select the type of resource you want to publish
 - Enter the required data:

![Publication](/img/publication.png)

 - Click the `Publish` button
 - After this, you will be provided with a URL where the resource will be accessible on the internet

## Command Line Utility

### Windows {#windows}

 - Download the archive: <a href={getUrl('windows', 'x86_64')}>{getFile('windows', 'x86_64')}</a>
 - Extract the archive to any folder. For convenience, we recommend extracting the archive and adding the folder path to the PATH variable.
 - Press `Win + R`
 - Type `cmd.exe` and press `Enter`
 - In the opened window, type `cd <path to folder>` and press `Enter`<sup>*</sup>

 <sup>*</sup> This step is not required if you added the folder path to the PATH variable.

#### Adding Path to PATH Variable

To add the folder path to the PATH variable, follow these steps:

 - Right-click on "My Computer" and select "Properties".
 - In the opened system properties window, click on "Advanced system settings" on the left.
 - In the system properties window, click the "Environment Variables" button.
 - In the environment variables window, under "System Variables" or "User Variables" select the "Path" variable and click "Edit".
 - In the environment variable edit window, click "New" and add the path to the folder where you extracted the archive. For example, if the path is `C:\path\to\your\folder`, add `C:\path\to\your\folder` as a new entry.
 - Click "OK" in all windows to save the changes.
 - To apply the changes, you may need to restart all open command prompt windows.

### Linux {#linux}

 - Open terminal
 - Download the archive using the command

<CodeBlock>wget {getUrl('linux', 'x86_64')}</CodeBlock>

 - Extract the archive using the command

<CodeBlock>tar -xvf {getFile('linux', 'x86_64')}</CodeBlock>

### MacOS {#macos}

 - Open terminal
 - Download and extract the archive for your architecture:

#### Apple Silicon:

<CodeBlock>curl {getUrl('macos', 'aarch64')} -o {getFile('macos', 'aarch64')}
tar -xvf {getFile('macos', 'aarch64')}</CodeBlock>

#### Intel:

<CodeBlock>curl {getUrl('macos', 'x86_64')} -o {getFile('macos', 'x86_64')}
tar -xvf {getFile('macos', 'x86_64')}</CodeBlock>

## Link Your Account

If you haven't done so already, create an account in the [personal account](https://cloudpub.online/dashboard). After that, link your account to the client by executing the following command:

```bash
clo set token <your token>
```

You can find your token on the main page in the personal account after registration.

Starting from client version 1.6, you can authenticate using the command:

```bash
clo login
```

:::note
On Linux and MacOS, you need to specify the path to the `clo` file, even if it's in the current directory:

```bash
./clo login
```
:::

## Publish Your First Resource

To publish a local HTTP server running on port 8080, execute the command:

```bash
clo publish http 8080
```

After this, you will be provided with a URL where your resource will be accessible on the internet, for example:

```bash
Service published: http://localhost:8080 -> https://wildly-suitable-fish.cloudpub.online
```

In this case, your resource will be accessible at `https://wildly-suitable-fish.cloudpub.online`.

Your resource URL will differ from the one shown in the example.
