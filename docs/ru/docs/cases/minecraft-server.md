---
sidebar_position: 9
description: Создание публичного сервера Minecraft через CloudPub
slug: /cases/minecraft-server
---

# Minecraft сервер

CloudPub позволяет создать публичный Minecraft сервер для игры с друзьями без настройки сети и проброса портов.

## Быстрый старт

### Использование протокола minecraft (автоматическая установка)

```bash
# Автоматически загружает и настраивает сервер Minecraft
clo publish minecraft /путь/к/папке/сервера

# Пример вывода:
# Service published: minecraft://C:\Minecraft -> minecraft://minecraft.cloudpub.ru:32123
# Игроки подключаются по адресу: minecraft.cloudpub.ru:32123
```

### Использование существующего сервера

```bash
# Запустите Minecraft сервер на порту 25565
# Затем опубликуйте через CloudPub
clo publish tcp 25565 --name minecraft

# Игроки подключаются по адресу:
# your-domain.cloudpub.ru:443
```

## Публикация сервера

### Java Edition
```bash
# Стандартный порт Minecraft Java Edition
clo publish tcp 25565 --name minecraft-java
```

### Bedrock Edition
```bash
# Minecraft Bedrock Edition использует UDP
clo publish udp 19132 --name minecraft-bedrock
```

### С плагинами (Spigot/Paper)
```bash
# Регистрация основного порта сервера
clo register tcp 25565 --name minecraft-main

# Публикация Dynmap и запуск всех сервисов
clo publish http 8123 --name minecraft-dynmap
```

## Множественные серверы

```bash
# Регистрируем Survival сервер
clo register tcp 25565 --name minecraft-survival

# Регистрируем Creative сервер
clo register tcp 25566 --name minecraft-creative

# Публикуем Minigames и запускаем все серверы
clo publish tcp 25567 --name minecraft-games

# Или запустить все серверы сразу
# clo run
```

## RCON управление

```bash
# Публикация RCON консоли для удаленного управления
clo publish tcp 25575 --name minecraft-rcon --auth basic
```

## Полезные команды

```bash
# Просмотр активных серверов
clo ls

# Остановка публикации сервера
clo stop minecraft

# Перезапуск туннеля
clo start minecraft
```

## Полезные ссылки

- [Сервер Minecraft](/docs/minecraft) - подробная документация
- [Публикация TCP сервисов](/docs/tcp) - TCP и UDP протоколы
- [Командная строка](/docs/cli) - все команды CLI
