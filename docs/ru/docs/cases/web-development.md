---
sidebar_position: 1
description: Локальная веб-разработка с CloudPub
slug: /cases/web-development
---

# Локальная веб-разработка

CloudPub позволяет мгновенно делиться доступом к локальным веб-серверам с коллегами и клиентами без деплоя на тестовый сервер.

## Быстрый старт

```bash
# Запустите ваш локальный сервер на порту 3000
# Затем опубликуйте его через CloudPub
clo publish http 3000

# С именем для удобства
clo publish http 3000 --name my-project

# С базовой аутентификацией
clo publish http 3000 --auth basic
```

## Примеры для популярных фреймворков

### React/Next.js
```bash
clo publish http 3000 --name react-app
```

### Django
```bash
clo publish http 8000 --name django-app
```

### Node.js/Express
```bash
clo publish http 5000 --name api-server
```

## Дополнительные возможности

```bash
# Публикация с кастомными заголовками
clo publish http 3000 --header "X-Custom:value"

# Публикация с правами доступа для конкретных пользователей
clo publish http 3000 --auth form --acl user@example.com:reader
```

## Полезные ссылки

- [Публикация HTTP сервисов](/docs/http) - подробная документация
- [Командная строка](/docs/cli) - все команды CLI
- [Аутентификация](/docs/auth) - настройка доступа
