const menu = document.querySelector<HTMLButtonElement>(".menu-toggle");
const nav = document.querySelector<HTMLElement>("#navigation");
if (menu && nav) {
  document.documentElement.classList.add("menu-ready");
  const close = () => menu.setAttribute("aria-expanded", "false");
  menu.addEventListener("click", () =>
    menu.setAttribute(
      "aria-expanded",
      String(menu.getAttribute("aria-expanded") !== "true"),
    ),
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && menu.getAttribute("aria-expanded") === "true") {
      close();
      menu.focus();
    }
  });
  nav.addEventListener("click", (e) => {
    if ((e.target as Element).closest("a")) close();
  });
}
document.querySelectorAll<HTMLAnchorElement>("[data-language]").forEach((a) =>
  a.addEventListener("click", () => {
    try {
      localStorage.setItem("epholang-language", a.dataset.language!);
    } catch {}
    const hash = location.hash;
    if (hash) a.href += hash;
  }),
);
if (document.body.dataset.root === "true") {
  let lang: string | null = null;
  try {
    lang = localStorage.getItem("epholang-language");
  } catch {}
  if (lang === "ru" || lang === "en") location.replace(`/epholang/${lang}/`);
  else {
    const preferred = navigator.language.startsWith("ru") ? "ru" : "en";
    document
      .querySelector(`[data-choice="${preferred}"]`)
      ?.classList.add("preferred");
  }
}
document.querySelectorAll<HTMLButtonElement>(".copy").forEach((button) => {
  button.hidden = false;
  button.addEventListener("click", async () => {
    const block = button.closest(".code-block")!;
    const status = block.querySelector<HTMLElement>(".copy-status")!;
    try {
      await navigator.clipboard.writeText(
        block.querySelector(".source code")!.textContent!,
      );
      status.textContent = button.dataset.success!;
      button.classList.add("done");
      setTimeout(() => button.classList.remove("done"), 2000);
    } catch {
      status.classList.remove("sr-only");
      status.textContent = button.dataset.failure!;
    }
  });
});
document.querySelectorAll<HTMLElement>("[data-tabs]").forEach((group) => {
  const buttons = [...group.querySelectorAll<HTMLButtonElement>("[data-tab]")];
  const panels = [...group.querySelectorAll<HTMLElement>("[data-panel]")];
  group.querySelector(".tab-list")?.setAttribute("role", "tablist");
  const activate = (index: number, focus = false) => {
    buttons.forEach((b, i) => {
      b.setAttribute("aria-selected", String(i === index));
      b.tabIndex = i === index ? 0 : -1;
      panels[i].hidden = i !== index;
    });
    if (focus) buttons[index].focus();
  };
  buttons.forEach((b, i) => {
    b.hidden = false;
    b.setAttribute("role", "tab");
    b.setAttribute("aria-controls", panels[i].id);
    panels[i].setAttribute("role", "tabpanel");
    panels[i].setAttribute("aria-labelledby", b.id);
    b.addEventListener("click", () => activate(i));
    b.addEventListener("keydown", (e) => {
      let next = i;
      if (e.key === "ArrowRight") next = (i + 1) % buttons.length;
      else if (e.key === "ArrowLeft")
        next = (i + buttons.length - 1) % buttons.length;
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = buttons.length - 1;
      else return;
      e.preventDefault();
      activate(next, true);
    });
  });
  activate(0);
});
