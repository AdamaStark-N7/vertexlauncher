(function () {
  if (window.__vertexSigninDiagnosticsInstalled) {
    return;
  }
  window.__vertexSigninDiagnosticsInstalled = true;

  function safeString(value) {
    if (typeof value === "string") {
      return value;
    }
    if (value === null || value === undefined) {
      return "";
    }
    try {
      return String(value);
    } catch (_) {
      return "";
    }
  }

  function emit(kind, detail) {
    try {
      if (!window.ipc || typeof window.ipc.postMessage !== "function") {
        return;
      }
      window.ipc.postMessage(JSON.stringify({
        kind: kind,
        href: safeString(window.location && window.location.href),
        title: safeString(document && document.title),
        readyState: safeString(document && document.readyState),
        hasBody: !!(document && document.body),
        visibilityState: safeString(document && document.visibilityState),
        detail: detail || null
      }));
    } catch (_) {}
  }

  emit("init", null);
  if (document) {
    document.addEventListener("readystatechange", function () {
      emit("readystatechange", { readyState: safeString(document.readyState) });
    });
    document.addEventListener("DOMContentLoaded", function () {
      emit("domcontentloaded", null);
    });
  }
  window.addEventListener("load", function () {
    emit("load", null);
  });
  window.addEventListener(
    "error",
    function (event) {
      emit("page-error", {
        message: safeString(event && event.message),
        filename: safeString(event && event.filename),
        lineno: event && event.lineno ? event.lineno : 0,
        colno: event && event.colno ? event.colno : 0
      });
    },
    true
  );
  window.addEventListener("unhandledrejection", function (event) {
    emit("unhandledrejection", {
      reason: safeString(event && event.reason)
    });
  });
  setTimeout(function () {
    emit("heartbeat-5s", null);
  }, 5000);
  setTimeout(function () {
    emit("heartbeat-15s", null);
  }, 15000);
})();
