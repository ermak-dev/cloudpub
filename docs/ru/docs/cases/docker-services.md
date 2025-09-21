---
sidebar_position: 4
description: Публикация Docker контейнеров и сервисов
slug: /cases/docker-services
---

# Docker сервисы

CloudPub может быть установлен через Docker образ для публикации как локальных сервисов с хост-машины, так и для интеграции в Docker Compose с другими контейнерами.

## Использование CloudPub через Docker образ

### Публикация локальных сервисов

```bash
# Публикация локального сервиса на порту 8080
docker run --rm --net=host -e TOKEN=your_token \
    cloudpub/cloudpub:latest publish http 8080

# Для macOS/Windows (где --net=host не работает)
docker run --rm -e TOKEN=your_token \
    cloudpub/cloudpub:latest publish http host.docker.internal:8080

# Публикация нескольких сервисов
docker run --rm --net=host -e TOKEN=your_token \
    -e HTTP=8080,3000 -e TCP=5432 \
    cloudpub/cloudpub:latest run
```

## CloudPub в Docker Compose

### Пример с веб-приложением

```yaml
services:
  webapp:
    image: nginx:alpine
    container_name: webapp
    expose:
      - "80"
    volumes:
      - ./html:/usr/share/nginx/html
    restart: always

  cloudpub:
    image: cloudpub/cloudpub:latest
    container_name: cloudpub
    environment:
      - TOKEN=your_cloudpub_token
      - HTTP=webapp:80
    command: run
    depends_on:
      - webapp
    restart: always
    volumes:
      - cloudpub-config:/home/cloudpub

volumes:
  cloudpub-config:
```

### Пример с несколькими сервисами

```yaml
services:
  frontend:
    image: node:18
    container_name: frontend
    working_dir: /app
    volumes:
      - ./frontend:/app
    command: npm start
    expose:
      - "3000"

  backend:
    image: python:3.11
    container_name: backend
    working_dir: /app
    volumes:
      - ./backend:/app
    command: python app.py
    expose:
      - "5000"

  cloudpub:
    image: cloudpub/cloudpub:latest
    container_name: cloudpub
    environment:
      - TOKEN=your_cloudpub_token
      - HTTP=frontend:3000,backend:5000
    command: run
    depends_on:
      - frontend
      - backend
    restart: always
    volumes:
      - cloudpub-config:/home/cloudpub

volumes:
  cloudpub-config:
```


## Запуск и управление

```bash
# Запуск всех сервисов
docker-compose up -d

# Просмотр логов CloudPub
docker-compose logs cloudpub

# Перезапуск туннеля
docker-compose restart cloudpub

# Остановка всех сервисов
docker-compose down
```

## Полезные ссылки

- [Docker установка](/docs/docker) - подробная документация по Docker
- [Командная строка](/docs/cli) - все команды CLI
- [TCP публикация](/docs/tcp) - публикация TCP сервисов
