Документация CloudPub Python SDK
==================================

CloudPub Python SDK предоставляет простой и мощный интерфейс для взаимодействия с
платформой сервисов CloudPub. CloudPub обеспечивает безопасное туннелирование и публикацию сервисов,
позволяя безопасно открывать локальные сервисы в интернет.

.. toctree::
   :maxdepth: 2
   :caption: Содержание:

   api_reference

Установка
---------

Установите SDK с помощью pip::

    pip install cloudpub-python-sdk

Быстрый старт
-------------

Простая публикация сервиса
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    from cloudpub_python_sdk import Connection, Protocol, Auth

    # Создание соединения с учетными данными
    conn = Connection(
        email="user@example.com",
        password="password"
    )

    # Публикация локального веб-сервиса
    endpoint = conn.publish(
        Protocol.HTTP,
        "localhost:3000",
        name="Мое веб-приложение",
        auth=Auth.NONE
    )

    print(f"Сервис опубликован по адресу: {endpoint.url}")

    # Поддержание сервиса в работе
    try:
        input("Нажмите Enter для остановки...")
    finally:
        # Очистка
        conn.unpublish(endpoint.guid)

Расширенная публикация сервиса
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    from cloudpub_python_sdk import (
        Connection, Protocol, Auth, Acl, Header,
        FilterRule, FilterAction, Role
    )

    conn = Connection(
        email="admin@example.com",
        password="password"
    )

    # Настройка списка контроля доступа
    acl = [
        Acl("admin@example.com", Role.ADMIN),
        Acl("user@example.com", Role.READER),
        Acl("writer@example.com", Role.WRITER),
    ]

    # Добавление пользовательских HTTP заголовков
    headers = [
        Header("X-API-Version", "2.0"),
        Header("Access-Control-Allow-Origin", "*"),
        Header("X-Custom-Header", "CloudPub"),
    ]

    # Определение правил фильтрации для обработки запросов
    rules = [
        FilterRule(
            data='http.path starts_with "/api/" and http.headers["X-API-Key"] contains "valid"',
            action_type=FilterAction.ALLOW,
            order=0
        ),
        FilterRule(
            data='http.path matches "^/admin.*" and not (ip.src >= 192.168.1.0 and ip.src <= 192.168.1.255)',
            action_type=FilterAction.DENY,
            order=1
        ),
        FilterRule(
            data='http.path starts_with "/api/v1/"',
            action_type=FilterAction.REDIRECT,
            action_value="https://api.example.com/v2/",
            order=2
        ),
    ]

    # Публикация сервиса со всеми расширенными возможностями
    endpoint = conn.publish(
        Protocol.HTTPS,
        "localhost:8443",
        name="Защищенный API сервер",
        auth=Auth.BASIC,
        acl=acl,
        headers=headers,
        rules=rules
    )

    print(f"Расширенный сервис опубликован по адресу: {endpoint.url}")

Типы сервисов
-------------

HTTP/HTTPS сервисы
~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Публикация HTTP сервиса с базовой аутентификацией
    http_endpoint = conn.publish(
        Protocol.HTTP,
        "localhost:8080",
        name="API сервер",
        auth=Auth.BASIC  # Требовать пароль для доступа
    )

    # Публикация HTTPS сервиса
    https_endpoint = conn.publish(
        Protocol.HTTPS,
        "localhost:8443",
        name="Защищенный API",
        auth=Auth.BASIC
    )

TCP/UDP сервисы
~~~~~~~~~~~~~~~

.. code-block:: python

    # Публикация TCP сервиса (например, SSH)
    tcp_endpoint = conn.publish(
        Protocol.TCP,
        "localhost:22",
        name="SSH сервер",
        auth=Auth.NONE
    )

    # Публикация UDP сервиса (например, DNS)
    udp_endpoint = conn.publish(
        Protocol.UDP,
        "localhost:53",
        name="DNS сервер",
        auth=Auth.NONE
    )

RTSP стриминговые сервисы
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Публикация RTSP потока с учетными данными в URL
    # Формат: rtsp://username:password@host:port/path
    rtsp_endpoint = conn.publish(
        Protocol.RTSP,
        "rtsp://camera:secret123@192.168.1.100:554/live/stream1",
        name="Камера безопасности",
        auth=Auth.BASIC  # Аутентификация доступа CloudPub
    )

    # RTSP без учетных данных (публичный поток)
    public_rtsp = conn.publish(
        Protocol.RTSP,
        "rtsp://localhost:554/public",
        name="Публичный поток",
        auth=Auth.NONE
    )

Управление сервисами
--------------------

Список сервисов
~~~~~~~~~~~~~~~

.. code-block:: python

    # Получить все зарегистрированные сервисы
    services = conn.ls()

    for service in services:
        print(f"Сервис: {service.guid}")
        print(f"  URL: {service.url}")
        print(f"  Статус: {service.status}")
        print(f"  Протокол: {service.remote_proto}")

Запуск и остановка сервисов
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Временно остановить сервис
    conn.stop("service-guid-123")

    # Перезапустить сервис
    conn.start("service-guid-123")

    # Постоянно удалить сервис
    conn.unpublish("service-guid-123")

    # Удалить все сервисы
    conn.clean()

Управление конфигурацией
------------------------

Динамическая конфигурация
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Установка значений конфигурации
    conn.set("server", "api.cloudpub.com")
    conn.set("port", "443")
    conn.set("ssl", "true")

    # Получение значений конфигурации
    server = conn.get("server")
    print(f"Сервер: {server}")

    # Получение всех опций конфигурации
    options = conn.options()
    for key, value in options.items():
        print(f"{key}: {value}")

Управление аутентификацией
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Аутентификация с учетными данными
    conn = Connection(
        email="user@example.com",
        password="password"
    )

    # Позже, выход для очистки токена
    conn.logout()

    # Повторная аутентификация с сохраненным токеном
    conn = Connection(token="saved-auth-token")

Вспомогательные функции
-----------------------

Проверка состояния сервера
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    # Измерение задержки сервера (возвращает микросекунды)
    latency_us = conn.ping()
    latency_ms = latency_us / 1000.0
    print(f"Задержка сервера: {latency_us}мкс ({latency_ms:.2f}мс)")

    if latency_ms > 100:
        print("Предупреждение: Обнаружена высокая задержка!")

Управление кэшем
~~~~~~~~~~~~~~~~

.. code-block:: python

    # Очистка локального кэша
    conn.purge()
    print("Кэш очищен")

Обработка ошибок
----------------

Все методы SDK генерируют соответствующие исключения для обработки ошибок:

.. code-block:: python

    from cloudpub_python_sdk import Connection, CloudPubError, AuthenticationError

    try:
        conn = Connection(
            email="user@example.com",
            password="wrong-password"
        )
    except AuthenticationError as e:
        print(f"Ошибка аутентификации: {e}")
        # Обработка ошибки аутентификации
    except CloudPubError as e:
        print(f"Ошибка соединения: {e}")
        # Обработка общей ошибки

Справочник по правилам фильтрации
----------------------------------

Правила фильтрации позволяют управлять маршрутизацией запросов и доступом на основе различных параметров соединения.
Правила обрабатываются по порядку (по полю ``order``), и первое подходящее правило применяется.

Доступные переменные
~~~~~~~~~~~~~~~~~~~~

- ``ip.src``, ``ip.dst`` - IP адреса источника и назначения
- ``port.src``, ``port.dst`` - Порты источника и назначения
- ``protocol`` - Протокол соединения ("tcp", "udp", "http")
- ``http.host``, ``http.path``, ``http.method`` - HTTP-специфичные поля
- ``http.headers["Header-Name"]`` - Доступ к HTTP заголовкам
- ``geo.country``, ``geo.region``, ``geo.city``, ``geo.code`` - Данные геолокации

Операторы
~~~~~~~~~

- **Сравнение**: ``==``, ``!=``, ``>``, ``<``, ``>=``, ``<=``
- **Строковые**: ``matches`` (regex), ``contains``, ``starts_with``, ``ends_with``
- **Логические**: ``and``, ``or``, ``not``

Примеры правил фильтрации
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

    rules = [
        # Разрешить доступ к API только из локальной сети
        FilterRule(
            data='ip.src >= 192.168.1.0 and ip.src <= 192.168.1.255 and http.path starts_with "/api/"',
            action_type=FilterAction.ALLOW,
            order=0
        ),
        # Блокировать доступ к админ-панели снаружи
        FilterRule(
            data='http.path matches "^/admin.*"',
            action_type=FilterAction.DENY,
            order=1
        ),
        # Перенаправление старых API эндпоинтов на новую версию
        FilterRule(
            data='http.path starts_with "/api/v1/"',
            action_type=FilterAction.REDIRECT,
            action_value="https://api.example.com/v2/",
            order=2
        ),
        # Блокировать запросы из определенных стран
        FilterRule(
            data='geo.country != "Россия"',
            action_type=FilterAction.DENY,
            order=3
        ),
        # Разрешить только аутентифицированные API запросы
        FilterRule(
            data='http.path starts_with "/api/" and http.headers["Authorization"] contains "Bearer"',
            action_type=FilterAction.ALLOW,
            order=4
        ),
    ]

Полный пример
-------------

Вот полный пример, демонстрирующий полный жизненный цикл с расширенными возможностями:

.. code-block:: python

    from cloudpub_python_sdk import (
        Connection, Protocol, Auth, Acl, Header,
        FilterRule, FilterAction, Role, CloudPubError
    )
    import time

    def main():
        try:
            # Инициализация соединения с конфигурацией
            print("Подключение к CloudPub...")
            conn = Connection(
                email="admin@example.com",
                password="secure-password",
                log_level="info",
                verbose=True
            )
            print("Успешно подключено!")

            # Публикация нескольких сервисов
            print("\nПубликация сервисов...")

            # Простой веб-сервис без расширенных возможностей
            web_service = conn.publish(
                Protocol.HTTP,
                "localhost:3000",
                name="Веб-приложение",
                auth=Auth.NONE
            )
            print(f"✓ Веб-приложение: {web_service.url}")

            # API сервер с ACL и пользовательскими заголовками
            api_acl = [
                Acl("api_admin@example.com", Role.ADMIN),
                Acl("api_user@example.com", Role.READER),
            ]
            api_headers = [
                Header("X-API-Version", "2.0"),
                Header("Access-Control-Allow-Origin", "*"),
            ]
            api_service = conn.publish(
                Protocol.HTTPS,
                "localhost:8443",
                name="API сервер",
                auth=Auth.BASIC,
                acl=api_acl,
                headers=api_headers
            )
            print(f"✓ API сервер: {api_service.url}")

            # SSH сервис с правилами фильтрации для безопасности
            ssh_rules = [
                FilterRule(
                    data="ip.src == 192.168.1.10 or ip.src == 10.0.0.5",
                    action_type=FilterAction.ALLOW,
                    order=0
                ),
                FilterRule(
                    data='ip.src matches ".*"',
                    action_type=FilterAction.DENY,
                    order=1
                ),
            ]
            ssh_service = conn.publish(
                Protocol.TCP,
                "localhost:22",
                name="SSH доступ",
                auth=Auth.BASIC,
                rules=ssh_rules
            )
            print(f"✓ SSH: {ssh_service.url}")

            # Проверка состояния сервера
            print("\nПроверка состояния сервера...")
            latency_us = conn.ping()
            print(f"Задержка сервера: {latency_us}мкс ({latency_us/1000.0:.2f}мс)")

            # Список всех сервисов
            print("\nАктивные сервисы:")
            services = conn.ls()
            for i, service in enumerate(services, 1):
                print(f"{i}. {service.guid} - {service.url}")

            # Работать некоторое время
            print("\nСервисы запущены. Нажмите Ctrl+C для остановки...")
            try:
                time.sleep(3600)  # Работать 1 час
            except KeyboardInterrupt:
                print("\nОстановка...")

            # Очистка
            print("Очистка сервисов...")
            for service in services:
                conn.unpublish(service.guid)
                print(f"✓ Удален: {service.guid}")

            print("Все сервисы остановлены. До свидания!")

        except CloudPubError as e:
            print(f"Ошибка: {e}")
            return 1

        return 0

    if __name__ == "__main__":
        exit(main())

Индексы и таблицы
=================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
