## Context
App de Windows para jugadores que sube la saturación de la pantalla por encima del tope de 100 que imponen Windows y los paneles de los fabricantes (como el de Intel), con una interfaz minimalista cuyos controles reaccionan.
Stack: Rust + Slint
Expensive decisions: ninguna

## Backlog
1. Como jugador, quiero subir la saturación de mi pantalla por encima de 100 con un control deslizante, para ver colores más vivos de lo que Windows o el panel de Intel permiten.
2. Como jugador, quiero ajustar el contraste desde la misma app, para definir mejor la imagen. Requires: #1
3. Como jugador, quiero ajustar el brillo desde la misma app, para aclarar u oscurecer la imagen sin ir a otro panel. Requires: #1
4. Como jugador, quiero que los controles reaccionen visualmente mientras los muevo, para percibir cada ajuste al instante. Requires: #1
5. Como jugador, quiero que mis ajustes se mantengan dentro de los juegos en pantalla completa, para verlos donde realmente me importan. Requires: #1
6. Como jugador competitivo, quiero comprobar que Steam y FACEIT no toman la app como hack mientras está activa, para no arriesgar mi cuenta. Requires: #5
7. Como jugador con placa de otra marca que la de mi PC (Intel, NVIDIA o AMD), quiero que la app funcione igual en la mía, para no depender del panel de cada fabricante. Requires: #1
8. Como jugador, quiero que la app consuma muy poca RAM y CPU mientras juego, para que no me cueste rendimiento. Requires: #1
9. Como jugador, quiero que la app recuerde mis valores al reiniciar la PC, para no tener que configurarlos cada vez. Requires: #1
10. Como jugador, quiero una forma rápida de volver a la imagen normal, para deshacer mis ajustes cuando no los quiero. Requires: #1

## Notas
- La #6 termina en evidencia de que la app no fue marcada como hack, no en una garantía: solo Valve y FACEIT pueden certificarlo.
