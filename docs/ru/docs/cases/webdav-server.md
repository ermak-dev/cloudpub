---
sidebar_position: 10
description: Создание WebDAV файлового сервера через CloudPub
slug: /cases/webdav-server
---

# WebDAV файловый сервер

CloudPub позволяет создать собственное облачное хранилище с доступом через WebDAV протокол.

## Быстрый старт

```bash
# Публикация WebDAV сервера
clo publish webdav /path/to/files --name my-cloud

# Подключение как сетевой диск:
# Windows: \\your-domain.cloudpub.ru@SSL\DavWWWRoot
# macOS: https://your-domain.cloudpub.ru
# Linux: davs://your-domain.cloudpub.ru
```

## Настройка доступа

### С паролем
```bash
# WebDAV с базовой аутентификацией
clo publish webdav /home/user/documents --auth basic --name documents
```

### С правами доступа
```bash
# Разные уровни доступа для пользователей
clo publish webdav /shared/files \
  --auth basic \
  --acl user1@example.com:reader \
  --acl user2@example.com:writer \
  --acl admin@example.com:admin \
  --name shared-storage
```

## Публикация разных директорий

```bash
# Документы
clo publish webdav /home/user/Documents --name docs

# Медиафайлы
clo publish webdav /home/user/Media --name media

# Резервные копии
clo publish webdav /backups --auth basic --name backups
```

## Синхронизация с клиентами

### Nextcloud/ownCloud клиенты
```bash
# Публикация для синхронизации
clo publish webdav /cloud/data --name nextcloud-sync
```

### Мобильные приложения
```bash
# Для iOS/Android WebDAV клиентов
clo publish webdav /mobile/sync --auth basic --name mobile-sync
```

## Полезные команды

```bash
# Просмотр активных WebDAV серверов
clo ls

# Изменение прав доступа
clo set acl --name my-cloud --acl newuser@example.com:writer

# Остановка и запуск
clo stop my-cloud
clo start my-cloud
```
