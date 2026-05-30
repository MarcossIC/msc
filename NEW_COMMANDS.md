

  2. El help overlay es una trampa modal
  render.rs:953-981 — El overlay dice "Press any key to close this help" pero no hay código que consuma input mientras el overlay está abierto. El  
  usuario queda atrapado.

  MEDIO — Mejoras de diseño y performance

  11. Recursión sin límite en cleaner
  cleaner.rs:218-280 — count_files_recursive() no tiene límite de profundidad. Un directorio con 10,000 niveles de anidamiento = stack overflow.    

  12. Solo 1 evento de input por frame
  app.rs:255-272 — Solo procesa UN Event::Key por iteración del loop. Si el usuario mantiene apretada una tecla, siente lag. Procesá todos los      
  eventos disponibles antes de renderizar.

  ---
  UI/UX — Lo que el usuario ve

  14. Selección se pierde al cambiar vista
  app.rs:175-178 — Al togglear entre tree/list view, selected_process_index se resetea a 0. El usuario pierde la posición de scroll.


  16. Mouse capturado pero no usado
  Mouse capture está habilitado pero no hay handler de Event::Mouse. El scroll con rueda del mouse no funciona.

  ---
  Resumen ejecutivo

  ┌───────────┬───────────────────────────────────────────┬──────────┐
  │ Prioridad │                    Qué                    │ Esfuerzo │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ ALTO      │ Tests para commands, platform, wget       │ Alto     │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ ALTO      │ Unificar validadores de alias             │ Medio    │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ ALTO      │ Fix TOCTOU en find_free_port              │ Medio    │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ MEDIO     │ Reducir allocaciones en render            │ Medio    │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ MEDIO     │ Versionado de config                      │ Medio    │
  ├───────────┼───────────────────────────────────────────┼──────────┤
  │ UX        │ Sort indicator, scroll position, mouse    │ Bajo     │
  └───────────┴───────────────────────────────────────────┴──────────┘
