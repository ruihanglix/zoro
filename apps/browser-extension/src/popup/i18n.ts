// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

/**
 * Lightweight i18n for the browser extension popup.
 * Shares the same translation keys as the desktop app's "browserExtension" namespace.
 */

type Translations = Record<string, Record<string, string>>;

const translations: Translations = {
  en: {
    "appNotRunning": "Zoro desktop app is not running. Please start it to save papers.",
    "noPaperDetected": "No paper detected on this page. Navigate to ArXiv, DOI, or a page with citation metadata.",
    "pdfAvailable": "PDF available",
    "htmlAvailable": "HTML available",
    "alsoSaveHtml": "Also save HTML version",
    "saveToZoro": "Save to Zoro",
    "saving": "Saving...",
    "savedToZoro": "Saved to Zoro!",
    "more": "+{{count}} more",
  },
  "zh-CN": {
    "appNotRunning": "Zoro 桌面应用未运行。请启动后再保存论文。",
    "noPaperDetected": "未在此页面检测到论文。请前往 ArXiv、DOI 或包含引用元数据的页面。",
    "pdfAvailable": "PDF 可用",
    "htmlAvailable": "HTML 可用",
    "alsoSaveHtml": "同时保存 HTML 版本",
    "saveToZoro": "保存到 Zoro",
    "saving": "正在保存...",
    "savedToZoro": "已保存到 Zoro!",
    "more": "+{{count}} 更多",
  },
  ja: {
    "appNotRunning": "Zoro デスクトップアプリが実行されていません。論文を保存するには起動してください。",
    "noPaperDetected": "このページで論文が検出されませんでした。ArXiv、DOI、または引用メタデータのあるページに移動してください。",
    "pdfAvailable": "PDF あり",
    "htmlAvailable": "HTML あり",
    "alsoSaveHtml": "HTML版も保存する",
    "saveToZoro": "Zoro に保存",
    "saving": "保存中...",
    "savedToZoro": "Zoro に保存しました！",
    "more": "+{{count}} 件",
  },
  ko: {
    "appNotRunning": "Zoro 데스크톱 앱이 실행되고 있지 않습니다. 논문을 저장하려면 시작해 주세요.",
    "noPaperDetected": "이 페이지에서 논문을 감지하지 못했습니다. ArXiv, DOI 또는 인용 메타데이터가 있는 페이지로 이동하세요.",
    "pdfAvailable": "PDF 사용 가능",
    "htmlAvailable": "HTML 사용 가능",
    "alsoSaveHtml": "HTML 버전도 저장",
    "saveToZoro": "Zoro에 저장",
    "saving": "저장 중...",
    "savedToZoro": "Zoro에 저장됨!",
    "more": "+{{count}}개 더",
  },
  es: {
    "appNotRunning": "La aplicación de escritorio Zoro no está en ejecución. Iníciela para guardar artículos.",
    "noPaperDetected": "No se detectó ningún artículo en esta página. Navegue a ArXiv, DOI o una página con metadatos de cita.",
    "pdfAvailable": "PDF disponible",
    "htmlAvailable": "HTML disponible",
    "alsoSaveHtml": "También guardar versión HTML",
    "saveToZoro": "Guardar en Zoro",
    "saving": "Guardando...",
    "savedToZoro": "¡Guardado en Zoro!",
    "more": "+{{count}} más",
  },
  fr: {
    "appNotRunning": "L'application de bureau Zoro n'est pas en cours d'exécution. Veuillez la démarrer pour enregistrer des articles.",
    "noPaperDetected": "Aucun article détecté sur cette page. Accédez à ArXiv, DOI ou une page avec des métadonnées de citation.",
    "pdfAvailable": "PDF disponible",
    "htmlAvailable": "HTML disponible",
    "alsoSaveHtml": "Enregistrer aussi la version HTML",
    "saveToZoro": "Enregistrer dans Zoro",
    "saving": "Enregistrement...",
    "savedToZoro": "Enregistré dans Zoro !",
    "more": "+{{count}} de plus",
  },
  de: {
    "appNotRunning": "Die Zoro-Desktop-App läuft nicht. Bitte starten Sie sie, um Artikel zu speichern.",
    "noPaperDetected": "Kein Artikel auf dieser Seite erkannt. Navigieren Sie zu ArXiv, DOI oder einer Seite mit Zitiermetadaten.",
    "pdfAvailable": "PDF verfügbar",
    "htmlAvailable": "HTML verfügbar",
    "alsoSaveHtml": "Auch HTML-Version speichern",
    "saveToZoro": "In Zoro speichern",
    "saving": "Speichern...",
    "savedToZoro": "In Zoro gespeichert!",
    "more": "+{{count}} weitere",
  },
  pt: {
    "appNotRunning": "O aplicativo de desktop Zoro não está em execução. Inicie-o para salvar artigos.",
    "noPaperDetected": "Nenhum artigo detectado nesta página. Navegue para ArXiv, DOI ou uma página com metadados de citação.",
    "pdfAvailable": "PDF disponível",
    "htmlAvailable": "HTML disponível",
    "alsoSaveHtml": "Também salvar versão HTML",
    "saveToZoro": "Salvar no Zoro",
    "saving": "Salvando...",
    "savedToZoro": "Salvo no Zoro!",
    "more": "+{{count}} mais",
  },
  ru: {
    "appNotRunning": "Приложение Zoro не запущено. Пожалуйста, запустите его для сохранения статей.",
    "noPaperDetected": "На этой странице не обнаружена статья. Перейдите на ArXiv, DOI или страницу с метаданными цитирования.",
    "pdfAvailable": "PDF доступен",
    "htmlAvailable": "HTML доступен",
    "alsoSaveHtml": "Также сохранить HTML-версию",
    "saveToZoro": "Сохранить в Zoro",
    "saving": "Сохранение...",
    "savedToZoro": "Сохранено в Zoro!",
    "more": "+{{count}} ещё",
  },
};

let currentLang = "en";

/** Detect language from browser or Chrome storage */
export function initLang(): Promise<string> {
  return new Promise((resolve) => {
    // Try Chrome storage first (synced with desktop app preference)
    if (typeof chrome !== "undefined" && chrome.storage?.local) {
      chrome.storage.local.get("zoro-ui-language", (result) => {
        const lang = result["zoro-ui-language"];
        if (lang && translations[lang]) {
          currentLang = lang;
        } else {
          // Fallback to browser language
          const browserLang = navigator.language;
          if (translations[browserLang]) {
            currentLang = browserLang;
          } else {
            const prefix = browserLang.split("-")[0];
            if (translations[prefix]) {
              currentLang = prefix;
            }
          }
        }
        resolve(currentLang);
      });
    } else {
      const browserLang = navigator.language;
      if (translations[browserLang]) {
        currentLang = browserLang;
      }
      resolve(currentLang);
    }
  });
}

/** Translate a key with optional interpolation */
export function t(key: string, params?: Record<string, string | number>): string {
  const dict = translations[currentLang] ?? translations.en;
  let value = dict[key] ?? translations.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      value = value.replace(`{{${k}}}`, String(v));
    }
  }
  return value;
}
