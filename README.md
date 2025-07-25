# CloudPub

Это репозиторий для клиентской части сервиса CloudPub, который является открытым и распространяется под лицензией Apache 2.0.

https://cloudpub.ru

[![Звезды на GitHub](https://img.shields.io/github/stars/ermak-dev/cloudpub)](https://github.com/ermak-dev/cloudpub/stargazers)
[![Лицензия](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Мощный, надежный и эффективный обратный прокси для преодоления NAT, разработанный на Rust

CloudPub, подобно ngrok (https://github.com/inconshreveable/ngrok), облегчает предоставление услуг устройств, находящихся за NAT, в Интернете, используя сервер с общедоступным IP-адресом.

## Что такое CloudPub

CloudPub – это отечественная альтернатива известному инструменту Ngrok, представляющая собой комбинацию прокси-сервера, шлюза и туннеля в локальную сеть. Его основная задача заключается в предоставлении публичного доступа к локальным ресурсам, таким как веб-приложения, базы данных, игровые сервера и другие сервисы, запущенные на вашем компьютере или в локальной сети.

## Документация

Пожалуйста, посмотрите https://cloudpub.ru/docs

## Сборка клиентов

### Сборка с помощью Docker

Для сборки клиентов для всех поддерживаемых архитектур используйте Docker:

```bash
docker build --target artifacts --output type=local,dest=. .
```

После выполнения команды в директории `artifacts` будут созданы клиенты для следующих архитектур:

- `artifacts/x86_64/clo` - Linux x86_64
- `artifacts/aarch64/clo` - Linux ARM64
- `artifacts/arm/clo` - Linux ARM
- `artifacts/armv5te/clo` - Linux ARMv5TE
- `artifacts/win64/clo.exe` - Windows x86_64

### Локальная сборка

Для локальной сборки клиента для текущей архитектуры:

```bash
cargo build -p client --release
```

Исполняемый файл будет создан в `target/release/client`.

### Кросс-компиляция

Для сборки под конкретную архитектуру установите соответствующий target и выполните сборку:

```bash
# Для ARM64
rustup target add aarch64-unknown-linux-musl
cargo build -p client --target aarch64-unknown-linux-musl --release --no-default-features

# Для Windows
rustup target add x86_64-pc-windows-gnu
cargo build -p client --target x86_64-pc-windows-gnu --release
```

## Благодарности

На основе [RatHole](https://github.com/rapiz1/rathole) от [Юджиа Цяо](https://github.com/rapiz1)
