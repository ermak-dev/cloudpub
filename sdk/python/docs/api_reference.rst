Справочник API
=============

В этом разделе представлена подробная документация API для всех классов, методов и функций,
доступных в CloudPub Python SDK.

Содержимое модуля
---------------

.. automodule:: cloudpub_python_sdk
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

Классы
------

Connection
~~~~~~~~~~

.. autoclass:: cloudpub_python_sdk.Connection
   :members:
   :undoc-members:
   :special-members: __init__
   :show-inheritance:
   :no-index:

ServerEndpoint
~~~~~~~~~~~~~~

.. autoclass:: cloudpub_python_sdk.ServerEndpoint
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

Перечисления
------------

Protocol
~~~~~~~~

.. autoclass:: cloudpub_python_sdk.Protocol
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

   Доступные протоколы:

   - ``HTTP`` - Протокол HTTP
   - ``HTTPS`` - Протокол HTTPS с SSL/TLS
   - ``TCP`` - Прямой TCP сокет
   - ``UDP`` - UDP датаграммный сокет
   - ``ONEC`` - Протокол 1С
   - ``MINECRAFT`` - Протокол сервера Minecraft
   - ``WEBDAV`` - Протокол WebDAV
   - ``RTSP`` - Протокол потоковой передачи в реальном времени

Auth
~~~~

.. autoclass:: cloudpub_python_sdk.Auth
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

   Типы аутентификации:

   - ``NONE`` - Аутентификация не требуется
   - ``BASIC`` - Базовая HTTP аутентификация
   - ``FORM`` - Аутентификация на основе форм

Role
~~~~

.. autoclass:: cloudpub_python_sdk.Role
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

   Роли пользователей для контроля доступа:

   - ``NOBODY`` - Нет доступа
   - ``ADMIN`` - Полный административный доступ
   - ``READER`` - Доступ только для чтения
   - ``WRITER`` - Доступ для чтения и записи

FilterAction
~~~~~~~~~~~~

.. autoclass:: cloudpub_python_sdk.FilterAction
   :members:
   :undoc-members:
   :show-inheritance:
   :no-index:

   Действия правил фильтрации:

   - ``ALLOW`` - Разрешить подключение
   - ``DENY`` - Запретить подключение
   - ``REDIRECT`` - Перенаправить на другой URL
   - ``LOG`` - Записать подключение в лог

Классы данных
-------------

Acl
~~~

.. autoclass:: cloudpub_python_sdk.Acl
   :members:
   :undoc-members:
   :special-members: __init__
   :show-inheritance:
   :no-index:

Header
~~~~~~

.. autoclass:: cloudpub_python_sdk.Header
   :members:
   :undoc-members:
   :special-members: __init__
   :show-inheritance:
   :no-index:

FilterRule
~~~~~~~~~~

.. autoclass:: cloudpub_python_sdk.FilterRule
   :members:
   :undoc-members:
   :special-members: __init__
   :show-inheritance:
   :no-index:

Исключения
----------

CloudPubError
~~~~~~~~~~~~~

.. autoexception:: cloudpub_python_sdk.CloudPubError
   :members:
   :show-inheritance:

   Базовое исключение для всех ошибок CloudPub SDK.

AuthenticationError
~~~~~~~~~~~~~~~~~~~

.. autoexception:: cloudpub_python_sdk.AuthenticationError
   :members:
   :show-inheritance:

   Возникает при сбое аутентификации или неверных учетных данных.

ConnectionError
~~~~~~~~~~~~~~~

.. autoexception:: cloudpub_python_sdk.ConnectionError
   :members:
   :show-inheritance:

   Возникает при сбое подключения к серверу CloudPub.

ConfigError
~~~~~~~~~~~

.. autoexception:: cloudpub_python_sdk.ConfigError
   :members:
   :show-inheritance:

   Возникает при сбое операций конфигурации.
