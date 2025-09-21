---
sidebar_position: 12
description: Организация совместной работы команды через CloudPub
slug: /cases/team-collaboration
---

# Командная работа

CloudPub позволяет организовать совместную работу команды с контролем доступа и аутентификацией.

## Быстрый старт

```bash
# Публикация сервиса для команды
clo publish http 3000 --name team-app \
  --auth form \
  --acl developer1@team.com:admin \
  --acl developer2@team.com:reader \
  --acl manager@team.com:reader
```

## Контроль доступа

### Уровни доступа
```bash
# admin - полный доступ
# reader - только чтение
# writer - чтение и запись (для WebDAV)

clo publish http 8080 --name project \
  --auth form \
  --acl lead@company.com:admin \
  --acl dev@company.com:reader
```

### Базовая аутентификация
```bash
# Единый пароль для всей команды
clo publish http 3000 --auth basic --name team-portal
```
