---
sidebar_position: 11
description: Удаленный доступ к Home Assistant через CloudPub
slug: /cases/home-assistant
---

# Home Assistant

CloudPub обеспечивает безопасный удаленный доступ к панели управления умным домом Home Assistant.

## Home Assistant Add-on

Для пользователей Home Assistant OS доступен неофициальный add-on CloudPub, который упрощает настройку удаленного доступа. Add-on автоматически настраивает туннель и интегрируется с Home Assistant.

:::warning
Данный add-on является неофициальным и не связан с CloudPub.
:::

[CloudPub Add-on для Home Assistant (неофициальный)](https://github.com/black-roland/hassio-addon-cloudpub)

## Home Assistant Docker Compose

```yaml
services:
  homeassistant:
    container_name: homeassistant
    image: homeassistant/home-assistant:stable
    volumes:
      - ./config:/config
      - /etc/localtime:/etc/localtime:ro
    environment:
      - TZ=Your/Timezone # Replace with your actual timezone, e.g., America/New_York
    restart: always
    network_mode: host # Recommended for device discovery and integrations
    ports:
      - 8123:8123

  cloudpub:
    container_name: cloudpub
    image: cloudpub/cloudpub:latest
    environment:
      - TOKEN=your_cloudpub_token # Замените на ваш токен
    command: publish http host.docker.internal:8123 --name homeassistant
    restart: always
    depends_on:
      - homeassistant
```

После запуска `docker-compose up -d` ваш Home Assistant будет доступен через CloudPub по защищенному URL.
