#!/usr/bin/env python3
"""
Пример CloudPub Python SDK

Этот пример демонстрирует полный рабочий процесс использования CloudPub Python SDK,
включая настройку соединения, публикацию сервисов, управление и очистку.

Предварительные требования:
1. Сервер CloudPub должен быть запущен и доступен
2. Действительные учетные данные (email и пароль)
3. Локальные сервисы для публикации (пример использует localhost порты)
"""

from cloudpub_python_sdk import Connection, Protocol, Auth, Role, FilterAction, Acl, Header, FilterRule

def main():
    print("1. Подключение к серверу")
    # Создание соединения с сервером CloudPub
    # Для аутентификации должен быть предоставлен либо токен, либо email/пароль
    conn = Connection(
        "/tmp/cloudpub.toml",  # Путь к файлу конфигурации
        "info",                # Уровень логирования
        True,                  # Выводить логи в stderr
        None,                  # Токен
        "admin@example.com",   # Email
        "test"                 # Пароль
    )

    # Регистрация нового HTTP сервиса
    print("2. Публикация нового HTTP сервиса...")
    endpoint = conn.publish(
        Protocol.HTTP,             # Протокол
        "localhost:8080",          # Локальный адрес
        "Тестовый HTTP сервис",    # Имя сервиса
        Auth.NONE                  # Метод аутентификации для доступа к сервису
    )

    print(f"  Сервис опубликован: {endpoint.url}")

    print("3. Публикация нового TCP сервиса...")
    endpoint = conn.publish(
        Protocol.TCP,              # Протокол
        "localhost:22",            # Локальный адрес (пример SSH порта)
        "Тестовый TCP сервис",     # Имя сервиса
        Auth.NONE                  # Аутентификация не требуется
    )

    print(f"  Сервис опубликован: {endpoint.url}")

    # Сохранить GUID для последующих операций
    service_guid = endpoint.guid

    # Пример: Публикация RTSP потока со встроенными учетными данными
    print("4. Публикация RTSP потока (пример)...")
    rtsp_endpoint = conn.publish(
        Protocol.RTSP,                                           # Протокол
        "rtsp://camera:password@192.168.1.100:554/live/stream1", # RTSP URL с учетными данными
        "Камера безопасности",                                   # Имя сервиса
        Auth.BASIC                                              # Аутентификация доступа CloudPub
    )
    print(f"  RTSP поток опубликован: {rtsp_endpoint.url}")

    # Пример: Публикация публичного RTSP потока (без учетных данных)
    print("5. Публикация публичного RTSP потока...")
    public_rtsp = conn.publish(
        Protocol.RTSP,             # Протокол
        "rtsp://localhost:8554/public",  # RTSP URL без учетных данных
        "Публичный поток",         # Имя сервиса
        Auth.NONE                  # Без аутентификации
    )
    print(f"  Публичный RTSP опубликован: {public_rtsp.url}")

    print("6. Список сервисов:")
    services = conn.ls()
    for service in services:
        print(f"  {service.guid}: {service}")

    # Запустить сервис
    print(f"7. Запуск сервиса {service_guid}...")
    conn.start(service_guid)
    print("   - Сервис запущен")

    # Получить список сервисов снова, чтобы увидеть изменение статуса
    print("8. Проверка статуса сервиса...")
    services = conn.ls()
    service = next((s for s in services if s.guid == service_guid), None)
    if service:
        print(f"   - Статус сервиса: {service.status or 'Неизвестно'}")

    # Демонстрация функции ping
    print("9. Проверка связи с сервером...")
    latency_us = conn.ping()
    latency_ms = latency_us / 1000.0
    print(f"   - Задержка сервера: {latency_us}μs ({latency_ms:.2f}мс)")

    # Остановить сервис
    print(f"10. Остановка сервиса {service_guid}...")
    conn.stop(service_guid)
    print("   - Сервис остановлен")

    # Отменить регистрацию сервиса
    print(f"11. Отмена регистрации сервиса {service_guid}...")
    conn.unpublish(service_guid)
    print("   - Регистрация сервиса отменена")

    # Также очистить RTSP сервисы
    conn.unpublish(rtsp_endpoint.guid)
    conn.unpublish(public_rtsp.guid)

    # Финальный список для подтверждения удаления
    print("12. Финальный список сервисов...")
    services = conn.ls()
    print(f"   - Осталось {len(services)} сервис(ов)")

    # Очистить все сервисы
    print("13. Очистка всех сервисов...")
    conn.clean()
    print("   - Все сервисы удалены")

    # Проверить очистку
    services = conn.ls()
    print(f"   Финальное количество: {len(services)} сервис(ов)")

    print("Демонстрация успешно завершена!")
    conn.logout()


if __name__ == "__main__":
    main()
