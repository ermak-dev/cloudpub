---
sidebar_position: 98
description: Использование CloudPub с Docker
slug: /docker
---

# Образ Docker

## Использование CloudPub с Docker

Вы можете использовать готовый [образ Docker](https://hub.docker.com/repository/docker/cloudpub/cloudpub/general) для агента CloudPub.

Пример команды для запуска туннеля на порт 8080 на хост-машине выглядит следующим образом:

```bash
docker run --net=host -it -e TOKEN=xyz cloudpub/cloudpub:latest publish http 8080
```

:::tip
Docker-версия использует те же параметры командной строки, что и обычная версия.
:::

:::warning
Для пользователей MacOS или Windows, опция `--net=host` не будет работать.
:::

Вам нужно будет использовать специальный URL host.docker.internal, как описано в [документации](https://docs.docker.com/desktop/mac/networking/#use-cases-and-workarounds) Docker.

Например, для публикации HTTP-сервера на порту 8080, запущенного на хост-машине, используйте следующую команду:

```bash
docker run --net=host -it -e TOKEN=xyz cloudpub/cloudpub:latest \
       publish http host.docker.internal:8080
```

## Сохранение настроек при перезапуске контейнера

При запуске контейнера, CloudPub создает новый агент и новый уникальный URL для доступа к туннелю.

Что бы сохранить настройки при перезапуске контейнера, следует создать том для хранения конфигурации и кеша:


```bash
docker volume create cloudpub-config
```

Затем, при запуске контейнера, следует использовать этот том:

```bash
docker run -v cloudpub-config:/home/cloudpub --net=host -it -e TOKEN=xyz \
              cloudpub/cloudpub:latest publish http 8080
```

В этом случае все настройки агента будут сохранены в томе `cloudpub-config` и будут доступны при следующем запуске контейнера.

## Публикация сразу нескольких ресурсов

Вы можете указать несколько ресурсов для публикации в переменных окружения, разделяя их запятыми:

```bash
docker run -v cloudpub-config:/home/cloudpub --net=host -it\
              -e TOKEN=xyz \
              -e HTTP=8080,8081 \
              -e HTTPS=192.168.1.1:80 \
              cloudpub/cloudpub:latest run
```

Названия переменной окружения совпадает с названием протокола. Доступны следующие протоколы:

 * HTTP
 * HTTPS
 * TCP
 * UDP
 * WEBDAV
 * MINECRAFT

## Использование с Docker Compose

Для более сложных сценариев развертывания рекомендуется использовать Docker Compose. Это позволяет легко управлять несколькими сервисами и их зависимостями.

Пример файла `docker-compose.yml`:

```yaml
services:
  # Пример веб-приложения
  web-app:
    image: nginx:alpine
    volumes:
      - ./html:/usr/share/nginx/html
    expose:
      - "80"

  # Пример API сервера
  api-server:
    image: node:alpine
    working_dir: /app
    volumes:
      - ./api:/app
    command: npm start
    expose:
      - "3000"

  # Агент CloudPub для публикации обоих сервисов
  cloudpub:
    image: cloudpub/cloudpub:latest
    restart: unless-stopped
    environment:
      - TOKEN=your_cloudpub_token_here
      - HTTP=web-app:80,api-server:3000
    command: run
    depends_on:
      - web-app
      - api-server
    volumes:
      - cloudpub-config:/home/cloudpub

volumes:
  cloudpub-config:
```

Для запуска всех сервисов используйте:

```bash
docker-compose up -d
```

## Обновление образа Docker

Для обновления CloudPub до последней версии выполните следующие команды:

```bash
# Остановить контейнер
docker stop <container_name>

# Загрузить новую версию
docker pull cloudpub/cloudpub:latest

# Запустить контейнер с новой версией
docker run -v cloudpub-config:/home/cloudpub --net=host -it -e TOKEN=xyz \
              cloudpub/cloudpub:latest publish http 8080
```

**Очистка неиспользуемых образов (опционально):**

```bash
# Удалить все неиспользуемые образы
docker image prune

# Удалить все неиспользуемые образы, включая те, которые не привязаны к контейнерам
docker image prune -a
```

При использовании Docker Compose:

```bash
# Остановить сервисы
docker-compose down

# Обновить образы
docker-compose pull

# Запустить с обновленными образами
docker-compose up -d
```

## Версия для ARM процессоров {#arm}

Для ARM процессоров доступен образ `cloudpub/cloudpub:latest-arm64`

:::warning
ARM версия пока не поддерживает протоколы WebDAV, 1C и Minecraft. Поддерживаются только HTTP, HTTPS, TCP и UDP.
:::
