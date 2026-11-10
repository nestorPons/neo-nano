// Copiar bloques de comandos. Mejora progresiva: sin JS, el texto sigue ahí.
document.querySelectorAll("[data-copy]").forEach(function (button) {
  button.addEventListener("click", function () {
    var target = document.getElementById(button.getAttribute("data-copy"));
    if (!target) return;
    var text = target.innerText.replace(/\n+$/, "");

    function done(ok) {
      var previous = button.textContent;
      button.textContent = ok ? "copiado ✓" : "copia manual";
      window.setTimeout(function () {
        button.textContent = previous;
      }, 1400);
    }

    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(
        function () {
          done(true);
        },
        function () {
          done(false);
        }
      );
      return;
    }

    // Navegadores sin Clipboard API (o fuera de un contexto seguro).
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "absolute";
    area.style.left = "-9999px";
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (error) {
      ok = false;
    }
    document.body.removeChild(area);
    done(ok);
  });
});
