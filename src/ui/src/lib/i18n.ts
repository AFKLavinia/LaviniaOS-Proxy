export type Lang = "tr" | "en";

export interface Translations {
  appName: string;
  appSubtitle: string;
  statusIdle: string;
  statusActive: string;
  statusDegraded: string;
  tapToStart: string;
  tapToStop: string;
  tapToStopBlocked: string;
  proxyEndpoint: string;
  ipAddress: string;
  port: string;
  endpointHint: string;
  udpOn: string;
  udpOff: string;
  udpRelayed: string;
  udpNeedPerm: string;
  udpUnsupported: string;
  linkWifi: string;
  linkUsb: string;
  linkEthernet: string;
  linkBluetooth: string;
  linkCellular: string;
  linkOther: string;
  interface: string;
  proxy: string;
  connection: string;
  unnamedConn: string;
  udp: string;
  enableUdp: string;
  statsDownload: string;
  statsUpload: string;
  statsPackets: string;
  statsDropped: string;
  statsTelemetry: string;
  settingsTitle: string;
  language: string;
  theme: string;
  themeDark: string;
  themeMidnight: string;
  themeOled: string;
  autostartLabel: string;
  autostartDesc: string;
  close: string;
  authTitle: string;
  authDesc: string;
  authUsername: string;
  authPassword: string;
  authCancel: string;
  authConnect: string;
  permTitle: string;
  permDesc: string;
  permNote: string;
  permNotNow: string;
  permGrant: string;
  permWaiting: string;
  quickServers: string;
  saveCurrent: string;
  savedEndpoints: string;
  emptyEndpoints: string;
  delete: string;
}

export const DICTIONARY: Record<Lang, Translations> = {
  tr: {
    appName: "Lavinia",
    appSubtitle: "L3 tünel istemcisi",
    statusIdle: "Boşta",
    statusActive: "Tünel Aktif",
    statusDegraded: "Proxy Erişilemez",
    tapToStart: "Tüneli başlatmak için dokun",
    tapToStop: "Tüneli durdurmak için dokun",
    tapToStopBlocked: "Proxy erişilemez — trafik engellendi. Durdurmak için dokun.",
    proxyEndpoint: "Hedef Proxy Noktası",
    ipAddress: "IP Adresi",
    port: "Port",
    endpointHint: "SOCKS5 · Tüm giden trafik bu uç nokta üzerinden yönlendirilir",
    udpOn: "UDP Açık",
    udpOff: "UDP Kapalı",
    udpRelayed: "Ses ve video paketleri aktarılıyor",
    udpNeedPerm: "Sistem izni gerekiyor",
    udpUnsupported: "Proxy üzerinde UDP desteği yok",
    linkWifi: "Wi-Fi Hotspot",
    linkUsb: "USB Bağlantısı",
    linkEthernet: "Ethernet",
    linkBluetooth: "Bluetooth",
    linkCellular: "Hücresel Veri",
    linkOther: "Ağ Bağlantısı",
    interface: "Arayüz",
    proxy: "Proxy",
    connection: "Bağlantı",
    unnamedConn: "İsimsiz Bağlantı",
    udp: "UDP",
    enableUdp: "UDP'yi Etkinleştir…",
    statsDownload: "İndirme",
    statsUpload: "Yükleme",
    statsPackets: "paket",
    statsDropped: "Düşen paketler",
    statsTelemetry: "Telemetri İstatistikleri",
    settingsTitle: "Ayarlar & Görünüm",
    language: "Arayüz Dili",
    theme: "Renk Teması",
    themeDark: "Derin Gece (Standart)",
    themeMidnight: "Siber Gece (Lavinia Indigo)",
    themeOled: "Saf Siyah (OLED)",
    autostartLabel: "Sistem Açılışında Başlat",
    autostartDesc: "Bilgisayar açıldığında uygulamayı otomatik başlat",
    close: "Kapat",
    authTitle: "Proxy Kimlik Doğrulaması",
    authDesc: "SOCKS5 sunucusu kullanıcı adı ve parola gerektiriyor.",
    authUsername: "Kullanıcı Adı",
    authPassword: "Şifre",
    authCancel: "İptal",
    authConnect: "Bağlan",
    permTitle: "UDP için Sistem İzni Gerekli",
    permDesc: "Discord sesli/görüntülü aramalar ve oyunlar UDP kullanır. Lavinia'nın bu paketleri yönlendirebilmesi için CAP_NET_ADMIN yetkisi gerekir. Sistem sizden şifrenizi doğrulamanızı isteyecektir.",
    permNote: "İzin verildikten sonra uygulama tek seferlik yeniden başlatılır.",
    permNotNow: "Daha Sonra",
    permGrant: "İzin Ver",
    permWaiting: "Yetki bekleniyor…",
    quickServers: "Kayıtlı Sunucular",
    saveCurrent: "Bu Noktayı Kaydet",
    savedEndpoints: "Hızlı Seçim",
    emptyEndpoints: "Henüz kayıtlı bir uç nokta yok.",
    delete: "Sil",
  },
  en: {
    appName: "Lavinia",
    appSubtitle: "L3 tunnel client",
    statusIdle: "Idle",
    statusActive: "Tunnel active",
    statusDegraded: "Proxy unreachable",
    tapToStart: "Tap to start tunnel",
    tapToStop: "Tap to stop tunnel",
    tapToStopBlocked: "Phone proxy unreachable — traffic is blocked. Tap to stop.",
    proxyEndpoint: "Phone proxy endpoint",
    ipAddress: "IP address",
    port: "Port",
    endpointHint: "SOCKS5 · all outbound traffic is routed through this endpoint",
    udpOn: "UDP on",
    udpOff: "UDP off",
    udpRelayed: "Voice & video are relayed",
    udpNeedPerm: "Permission required",
    udpUnsupported: "Phone proxy has no UDP support",
    linkWifi: "Wi-Fi hotspot",
    linkUsb: "USB tethering",
    linkEthernet: "Ethernet",
    linkBluetooth: "Bluetooth",
    linkCellular: "Cellular",
    linkOther: "Network link",
    interface: "Interface",
    proxy: "Proxy",
    connection: "Connection",
    unnamedConn: "Unnamed connection",
    udp: "UDP",
    enableUdp: "Enable UDP…",
    statsDownload: "Download",
    statsUpload: "Upload",
    statsPackets: "packets",
    statsDropped: "Dropped packets",
    statsTelemetry: "Telemetry Statistics",
    settingsTitle: "Settings & Appearance",
    language: "Interface Language",
    theme: "Theme Accent",
    themeDark: "Deep Space (Standard)",
    themeMidnight: "Cyber Indigo (Lavinia)",
    themeOled: "Pure Black (OLED)",
    autostartLabel: "Launch at Login",
    autostartDesc: "Start Lavinia Client automatically when you log in",
    close: "Close",
    authTitle: "Proxy Authentication",
    authDesc: "SOCKS5 server requires a username and password.",
    authUsername: "Username",
    authPassword: "Password",
    authCancel: "Cancel",
    authConnect: "Connect",
    permTitle: "Permission needed for UDP",
    permDesc: "Discord voice, video calls and games use UDP. To relay them, Lavinia needs the network capability CAP_NET_ADMIN. Your system will ask for authorization.",
    permNote: "The app restarts once after permission is granted. Reconnect afterwards.",
    permNotNow: "Not now",
    permGrant: "Grant permission",
    permWaiting: "Waiting for authorization…",
    quickServers: "Saved Endpoints",
    saveCurrent: "Save Current Endpoint",
    savedEndpoints: "Quick Select",
    emptyEndpoints: "No saved endpoints yet.",
    delete: "Delete",
  },
};
