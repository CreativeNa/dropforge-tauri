/**
 * Preload для окон с twitch.tv: отключает WebAuthn/пасс-ключи.
 *
 * Twitch на странице входа сам вызывает navigator.credentials.get(), из-за чего
 * Windows показывает системное окно «Выберите ключ доступа». Приложению вход
 * по пасс-ключу не нужен, поэтому API убираем до загрузки скриптов страницы.
 *
 * Загружается с contextIsolation: false — только для окон, где открыт Twitch.
 */
(() => {
  try {
    const deny = () => {
      throw new DOMException('Passkeys are disabled in DropForge', 'NotAllowedError');
    };

    Object.defineProperty(window, 'PublicKeyCredential', {
      configurable: true,
      get: () => undefined,
    });

    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      get: () => ({
        get: async () => deny(),
        create: async () => deny(),
        preventSilentAccess: async () => {},
        store: async () => deny(),
      }),
    });

    if (navigator.credentials && 'credentials' in navigator) {
      delete navigator.credentials;
    }
  } catch {
    /* если API уже не переопределить — просто продолжаем */
  }
})();
