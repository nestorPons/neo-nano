// On-device translation for the site, same approach as nestorpons.com:
// the browser's Translator API (Chrome), no key, no server and no cost.
//
// English is the source language. The selector only offers the languages the
// API can actually reach on this browser; if none is available the switcher
// stays hidden and the site remains in English. The choice is remembered in
// localStorage. Arabic switches the document to right-to-left.
(function () {
  "use strict";

  var SOURCE = "en";
  var STORAGE_KEY = "nnano-site-lang";
  var RTL = { ar: true, he: true, fa: true, ur: true };

  var select = document.getElementById("lang");
  var wrap = document.querySelector("[data-lang-switch]");
  var status = document.querySelector(".lang-status");
  if (!select || !wrap) return;

  var elements = [];
  var attrs = [];
  var current = null;
  var currentLang = SOURCE;
  var runId = 0;
  var originalTitle = document.title;

  function store(key, value) {
    try {
      window.localStorage.setItem(key, value);
    } catch (error) {
      /* private mode: the choice just is not remembered */
    }
  }

  function recall(key) {
    try {
      return window.localStorage.getItem(key);
    } catch (error) {
      return null;
    }
  }

  function setStatus(text) {
    if (status) status.textContent = text || "";
  }

  function escapeHtml(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function escapeRegExp(text) {
    return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  // Translate a block but keep its inline <code>/<kbd> as code: the tokens
  // travel through the translator as plain words and are wrapped again on the
  // way back, with the original tag. Translations usually leave identifiers
  // and chords untouched.
  function withCodeTokens(el, translated) {
    var tags = {};
    Array.prototype.forEach.call(el.querySelectorAll("code, kbd"), function (node) {
      var token = node.textContent.trim();
      if (!token) return;
      tags[escapeHtml(token)] = node.tagName.toLowerCase();
    });

    var tokens = Object.keys(tags).sort(function (a, b) {
      return b.length - a.length;
    });

    var html = escapeHtml(translated);
    if (!tokens.length) return html;

    var pattern = new RegExp(
      "(" + tokens.map(function (token) { return escapeRegExp(token); }).join("|") + ")",
      "g"
    );
    return html
      .split(pattern)
      .map(function (part, index) {
        if (index % 2 !== 1) return part;
        var tag = tags[part] || "code";
        return "<" + tag + ">" + part + "</" + tag + ">";
      })
      .join("");
  }

  function collect() {
    elements = [];
    attrs = [];

    Array.prototype.forEach.call(document.querySelectorAll("[data-i18n]"), function (el) {
      var text = el.textContent.replace(/\s+/g, " ").trim();
      elements.push({
        el: el,
        text: text,
        html: el.innerHTML,
        tag: el.tagName,
      });
    });

    Array.prototype.forEach.call(document.querySelectorAll("[data-i18n-attr]"), function (el) {
      var name = el.getAttribute("data-i18n-attr");
      if (!name) return;
      attrs.push({ el: el, name: name, value: el.getAttribute(name) || "" });
    });
  }

  function applyTranslation(item, translated) {
    if (item.tag === "TITLE") {
      item.el.textContent = translated;
      return;
    }
    item.el.innerHTML = withCodeTokens(item.el, translated);
  }

  function restoreAll() {
    elements.forEach(function (item) {
      if (item.tag === "TITLE") item.el.textContent = item.text;
      else item.el.innerHTML = item.html;
    });
    attrs.forEach(function (item) {
      item.el.setAttribute(item.name, item.value);
    });
    document.title = originalTitle;
  }

  async function createTranslator(target, run) {
    if (!("Translator" in window)) return null;
    try {
      var availability = await window.Translator.availability({
        sourceLanguage: SOURCE,
        targetLanguage: target,
      });
      if (availability === "no" || availability === "unavailable") return null;
      if (run !== runId) return null;

      return await window.Translator.create({
        sourceLanguage: SOURCE,
        targetLanguage: target,
        monitor: function (monitor) {
          monitor.addEventListener("downloadprogress", function (event) {
            if (run !== runId) return;
            var percent = Math.round((event.loaded || 0) * 100);
            setStatus("downloading " + percent + "%");
          });
        },
      });
    } catch (error) {
      return null;
    }
  }

  async function translateAll(run) {
    for (var i = 0; i < elements.length; i++) {
      if (run !== runId || currentLang === SOURCE) return;
      var item = elements[i];
      if (!item.text) continue;
      try {
        var translated = await current.translate(item.text);
        if (run !== runId || currentLang === SOURCE) return;
        applyTranslation(item, translated);
      } catch (error) {
        /* leave this block in English */
      }
    }

    for (var j = 0; j < attrs.length; j++) {
      if (run !== runId || currentLang === SOURCE) return;
      var attr = attrs[j];
      if (!attr.value) continue;
      try {
        var value = await current.translate(attr.value);
        if (run !== runId || currentLang === SOURCE) return;
        attr.el.setAttribute(attr.name, value);
      } catch (error) {
        /* leave the attribute as it was */
      }
    }
  }

  function applyDirection(lang) {
    document.documentElement.lang = lang === "zh" ? "zh-Hans" : lang;
    document.documentElement.dir = RTL[lang] ? "rtl" : "ltr";
  }

  async function setLanguage(lang) {
    if (!lang || !select.querySelector('option[value="' + lang + '"]')) lang = SOURCE;
    var run = ++runId;
    currentLang = lang;
    store(STORAGE_KEY, lang);
    select.value = lang;
    applyDirection(lang);

    if (lang === SOURCE) {
      select.disabled = false;
      restoreAll();
      setStatus("");
      return;
    }

    setStatus("translating…");
    select.disabled = true;
    var translator = await createTranslator(lang, run);
    if (run !== runId) return;

    if (!translator) {
      currentLang = SOURCE;
      current = null;
      store(STORAGE_KEY, SOURCE);
      select.value = SOURCE;
      applyDirection(SOURCE);
      restoreAll();
      setStatus("");
      select.disabled = false;
      return;
    }

    current = translator;
    await translateAll(run);
    if (run !== runId) return;
    select.disabled = false;
    setStatus("");
  }

  // Hide the languages this browser cannot offer and the whole switcher when
  // there are none. Then restore the remembered choice.
  async function refreshLanguages() {
    if (!("Translator" in window)) return;

    var available = 0;
    var options = Array.prototype.slice.call(select.options);
    for (var i = 0; i < options.length; i++) {
      var option = options[i];
      if (option.value === SOURCE) continue;
      var ok = false;
      try {
        var state = await window.Translator.availability({
          sourceLanguage: SOURCE,
          targetLanguage: option.value,
        });
        ok = state !== "no" && state !== "unavailable";
      } catch (error) {
        ok = false;
      }
      option.hidden = !ok;
      option.disabled = !ok;
      if (ok) available++;
    }

    if (available === 0) return;
    wrap.hidden = false;

    var saved = recall(STORAGE_KEY);
    if (saved && saved !== SOURCE) {
      var chosen = select.querySelector('option[value="' + saved + '"]');
      if (chosen && !chosen.hidden && !chosen.disabled) {
        await setLanguage(saved);
      }
    }
  }

  select.addEventListener("change", function () {
    void setLanguage(select.value);
  });

  collect();
  if ("Translator" in window) {
    refreshLanguages();
  }
})();
