---
sidebar_position: 13
description: Фильтрация и контроль сетевого трафика в CloudPub
slug: /cases/traffic-filtering
---

# Управление трафиком

CloudPub предоставляет мощные возможности фильтрации трафика для безопасности и оптимизации.

## Быстрый старт

```bash
# Публикация с базовой фильтрацией по IP
clo publish http 3000 --filter "ip.src == 192.168.1.0/24" --name filtered-app

# Блокировка конкретной страны
clo publish http 8080 --filter "geo.country != 'CN'" --name geo-filtered
```

## Фильтрация по IP-адресам

```bash
# Разрешить только офисную сеть
clo publish http 3000 --filter "ip.src == 85.26.146.0/24" --name office-only

# Блокировать конкретные IP
clo publish http 8080 --filter "ip.src != 1.2.3.4" --name blocked-ip

# Множественные условия
clo publish http 3000 --filter "ip.src in [192.168.1.0/24, 10.0.0.0/8]" --name multi-network
```

## Геолокация

```bash
# Только для России
clo publish http 3000 --filter "geo.country == 'RU'" --name ru-only

# Блокировка стран
clo publish http 8080 --filter "geo.country not in ['CN', 'KP']" --name geo-block

# Региональный доступ
clo publish http 3000 --filter "geo.region == 'Moscow'" --name moscow-only
```

## HTTP фильтрация

```bash
# Только GET запросы
clo publish http 3000 --filter "http.method == 'GET'" --name read-only

# Защита админки
clo publish http 8080 --filter "http.path !~ '/admin.*'" --name no-admin

# Фильтрация по заголовкам
clo publish http 3000 --filter "http.headers['User-Agent'] ~ 'Mozilla.*'" --name browser-only
```

## Комбинированные правила

```bash
# IP + метод
clo publish http 3000 --filter "ip.src == 192.168.1.0/24 and http.method in ['GET', 'POST']" --name office-api

# Гео + путь
clo publish http 8080 --filter "geo.country == 'RU' and http.path !~ '/api/.*'" --name ru-web

# Сложная логика
clo publish http 3000 --filter "(ip.src == 10.0.0.0/8 or geo.country == 'RU') and http.method != 'DELETE'" --name complex-filter
```

## Управление правилами

```bash
# Обновление фильтра
clo set filter --name my-app --filter "ip.src == 192.168.0.0/16"

# Просмотр текущего фильтра
clo get filter --name my-app

# Удаление фильтра
clo set filter --name my-app --filter ""
```
