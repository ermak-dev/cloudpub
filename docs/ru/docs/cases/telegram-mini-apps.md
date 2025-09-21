---
sidebar_position: 3
description: Разработка и тестирование Telegram Mini Apps локально
slug: /cases/telegram-mini-apps
---

# Telegram Mini Apps

CloudPub упрощает разработку и тестирование Telegram Mini Apps (Web Apps), позволяя запускать веб-приложения локально и получать доступ к ним прямо из Telegram.

## Быстрый старт

```bash
# Запустите ваше веб-приложение локально
# Затем опубликуйте через CloudPub
clo publish http 3000 --name my-telegram-app

# Используйте полученный URL в BotFather
# при настройке Menu Button или Inline Button
# https://your-domain.cloudpub.ru
```

## Пример создания Mini App

### Шаг 1: Создание простого приложения

Создайте файл `index.html`:

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>My Telegram Mini App</title>
    <script src="https://telegram.org/js/telegram-web-app.js"></script>
</head>
<body>
    <h1>Hello, Telegram!</h1>
    <p id="user-info">Loading...</p>
    <button onclick="window.Telegram.WebApp.close()">Закрыть</button>

    <script>
        // Инициализация
        const tg = window.Telegram.WebApp;
        tg.ready();

        // Показ информации о пользователе
        const user = tg.initDataUnsafe?.user;
        if (user) {
            document.getElementById('user-info').textContent =
                `Привет, ${user.first_name}!`;
        }
    </script>
</body>
</html>
```

### Шаг 2: Запуск локального сервера

Создайте простой HTTP сервер с Python:

```bash
# В директории с index.html
python3 -m http.server 8000
```

### Шаг 3: Публикация через CloudPub

```bash
# Публикуем локальный сервер
clo publish http 8000 --name my-telegram-miniapp

# Получаем URL вида:
# https://my-telegram-miniapp.cloudpub.ru
```

### Шаг 4: Настройка бота

1. Откройте @BotFather в Telegram
2. Выберите вашего бота или создайте нового: `/newbot`
3. Настройте Mini App:
   ```
   /mybots → выберите бота → Bot Settings → Menu Button
   → Specify URL → вставьте URL от CloudPub
   ```

4. Или добавьте inline кнопку в боте:
   ```python
   # Пример для python-telegram-bot
   from telegram import InlineKeyboardButton, InlineKeyboardMarkup

   keyboard = [[
       InlineKeyboardButton(
           "Открыть Mini App",
           web_app={"url": "https://my-telegram-miniapp.cloudpub.ru"}
       )
   ]]
   reply_markup = InlineKeyboardMarkup(keyboard)
   ```

### Шаг 5: Тестирование

1. Откройте чат с вашим ботом
2. Нажмите на кнопку меню (рядом с полем ввода)
3. Mini App откроется прямо в Telegram
4. Используйте инспектор трафика CloudPub для отладки
