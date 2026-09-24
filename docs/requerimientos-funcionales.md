# Kilo12 — Requerimientos Funcionales

**Versión:** 1.4
**Fecha:** 2026-09-22
**Estado:** Alcance funcional cerrado. Todas las decisiones de modelo de datos están resueltas.

**Cambios en 1.4:** se incorpora la **libra** como unidad base y se sustituye la moneda única por tres métodos de pago —efectivo en pesos, transferencia y efectivo en dólares—, con tasa de cambio, arqueo por moneda y dos modos de cierre. Nuevo módulo §6.11 — Divisa y tasa de cambio.
**Cambios en 1.3:** se declaran las plataformas objetivo, que hasta ahora no estaban especificadas: Windows como principal y macOS como secundaria (RNF-8 a RNF-10).
**Cambios en 1.2:** la comisión del operador de caja se calcula sobre la **venta total** de la sesión, no sobre la ganancia, y se **descuenta de la ganancia bruta** del local. Se incorpora el concepto de ganancia neta de comisión.
**Cambios en 1.1:** se agrega el módulo de Comisión del operador de caja. Se cierra D-6 (base de cálculo y congelamiento del porcentaje).
**Cambios en 1.0:** se cierran D-3 (**una única vitrina**), D-4 (**presentaciones de venta con precio propio**) y D-5 (**anulación limitada al turno vigente**). Se agrega el módulo §6.2 — Unidades y presentaciones.
**Cambios en 0.2:** se cierran D-1 (valoración por **costo promedio ponderado**) y D-2 (**sin control de lotes ni caducidad**). Se agrega el módulo de Valoración del inventario.

---

## 1. Contexto y objetivo

Kilo12 es una aplicación de escritorio de **punto de venta y control de inventario** para un mercadito (tienda de abarrotes de barrio), construida con **Rust + Tauri**.

El objetivo del sistema es que el dueño pueda responder, en cualquier momento y sin conexión a internet, estas preguntas:

1. ¿Qué tengo, cuánto tengo y dónde está (bodega o vitrina)?
2. ¿Qué me costó y a cuánto lo estoy vendiendo? ¿Cuánto estoy ganando realmente?
3. ¿Qué se está vendiendo, qué está parado y qué se me está por acabar?
4. ¿Cuánto dinero entró hoy y cuadra con lo que hay en la caja?
5. ¿Cuánto le corresponde cobrar a quien atendió la caja hoy?

Todo lo demás es secundario. Si un requerimiento no sirve a una de esas cinco preguntas, es candidato a salir del alcance de la v1.

---

## 2. Alcance del sistema

### 2.1 Dentro del alcance (v1)

- Catálogo de productos con soporte a venta por unidad y **a granel** (peso/volumen).
- Control de inventario en **dos ubicaciones**: bodega y vitrina.
- Registro de compras/entradas de mercancía con costo.
- Venta de contado con búsqueda de productos por nombre en pantalla.
- Cobro en efectivo en pesos, transferencia y efectivo en dólares, con tasa de cambio configurable.
- Cierre y arqueo de caja diario, con conteo separado por moneda.
- Cálculo de la comisión del operador de caja como porcentaje de la venta de la sesión, y su descuento de la ganancia del local.
- Registro de mermas y ajustes de inventario.
- Gestión de precios de venta y cálculo de margen.
- Estadísticas e informes operativos del negocio.
- Respaldo y restauración de la base de datos local.

### 2.2 Fuera del alcance (v1)

| Excluido | Motivo |
|---|---|
| Venta fiada / cuentas por cobrar | El negocio opera solo de contado (confirmado). |
| Facturación fiscal, series, reporte tributario | No hay obligación fiscal; el ticket es comprobante interno. |
| Multiusuario, roles y permisos diferenciados | Un solo operador: el dueño. |
| Multi-terminal / sincronización entre cajas | Una sola caja. |
| Lector de código de barras, impresora térmica, balanza conectada | Operación manual confirmada. El diseño **no debe impedir** agregarlos después (ver RNF-4). |
| Sincronización a la nube, multi-sucursal, e-commerce | Contradice el requisito de operación 100% offline. |
| Gestión de proveedores como módulo completo (cuentas por pagar) | Solo se registra el proveedor como dato de la compra. |
| Control de lotes y fechas de caducidad | Decisión D-2 cerrada: el mercadito no comercializa perecederos. La existencia es una cantidad por ubicación, no un conjunto de lotes. |
| Valoración FIFO o LIFO del inventario | Decisión D-1 cerrada a favor del costo promedio ponderado. FIFO exige lotes, que quedan fuera del alcance. |
| Múltiples ubicaciones de exhibición | Decisión D-3 cerrada: existe una única vitrina. Las ubicaciones son exactamente dos y fijas: bodega y vitrina. |
| Anulación de ventas de turnos ya cerrados | Decisión D-5 cerrada: preservar la integridad del arqueo histórico. El caso se atiende mediante devolución (RF-VTA-16), no mediante anulación. |

---

## 3. Supuestos y restricciones

- **R-1 — Operación 100% offline.** La aplicación debe ser completamente funcional sin conexión a internet, en todo momento. No puede existir ninguna funcionalidad que dependa de un servicio remoto para operar.
- **R-2 — Almacenamiento local.** Todos los datos residen en el equipo del usuario. No hay servidor.
- **R-3 — Un único operador del sistema.** La aplicación no tiene cuentas de usuario ni permisos diferenciados; quien la usa ve costos, márgenes y estadísticas sin restricción. Que una empleada atienda la caja no cambia esto: su nombre se registra como un dato de la sesión (RF-CAJ-09), no como una cuenta con credenciales. Ver riesgo RI-3.
- **R-4 — Moneda del negocio y divisa aceptada.** El negocio lleva sus cuentas en **pesos cubanos (CUP)**: todos los precios, costos, márgenes, comisiones e informes se expresan en esa moneda. Se acepta además **dólar estadounidense (USD)** como forma de pago, convertido mediante una tasa de cambio que fija el propietario. No existen precios en USD: la divisa interviene únicamente en el momento del cobro.
- **R-5 — Un solo local.** Una instalación = un mercadito.
- **R-6 — El equipo puede fallar.** Al no haber respaldo en la nube, la pérdida del disco implica pérdida total del historial. El respaldo local es un requerimiento funcional de primera clase, no un extra.
- **R-7 — No hay productos perecederos.** El catálogo no incluye artículos con fecha de caducidad que deba controlarse. En consecuencia, la existencia de un producto es una cantidad por ubicación y no un conjunto de lotes diferenciados. Todas las unidades de un mismo producto son intercambiables entre sí.
- **R-8 — Valoración por costo promedio ponderado.** El inventario se valora mediante costo promedio ponderado a nivel de producto. Las reglas completas están en §6.6.
- **R-9 — La existencia se lleva en una única unidad base por producto.** Un producto puede venderse en varias presentaciones (suelto, paquete, caja), pero su inventario y su costo se registran siempre en la unidad base. Las presentaciones son formas de comprar y vender, no existencias separadas. Las reglas completas están en §6.2.
- **R-10 — Dos ubicaciones fijas.** El sistema reconoce exactamente dos ubicaciones de stock: bodega y vitrina. No son configurables ni extensibles en v1.
- **R-11 — El vuelto se entrega siempre en pesos**, cualquiera que sea la moneda con la que el cliente haya pagado.
- **R-12 — El arqueo no mezcla monedas.** Al cerrar, los pesos y los dólares se cuentan y se comparan por separado. Consolidarlos en un único saldo impediría detectar un faltante en una de las dos monedas.

---

## 4. Glosario

| Término | Definición operativa en este sistema |
|---|---|
| **Producto** | Artículo comercializable identificado de forma única (SKU). Ej.: "Arroz blanco a granel", "Refresco 500 ml". |
| **SKU** | Código interno único del producto, generado por el sistema o escrito por el usuario. |
| **Unidad base** | Unidad en la que se registran la existencia y el costo de un producto: unidad, kg, **lb**, g, L, ml. Es única por producto y no cambia. Toda cantidad de inventario está expresada en ella. |
| **Libra** | Unidad base habitual para la venta a granel. Es una unidad por derecho propio, no una presentación sobre el kilogramo: convertir entre ambas en cada operación introduciría un redondeo que se acumula. |
| **Tasa de cambio** | Pesos cubanos que equivalen a un dólar. La fija el propietario y queda registrada en cada cobro en divisa. |
| **Modo de cierre** | Forma de presentar los dólares al cerrar la caja: **separado** (los dólares aparte, en su moneda) o **consolidado** (convertidos a pesos y sumados al total). |
| **Presentación** | Forma comercial en que se vende un producto, con su propio precio: "unidad suelta", "six-pack", "caja de 24". Cada presentación tiene un factor de conversión a la unidad base. |
| **Factor de conversión** | Cuántas unidades base equivalen a una presentación. Un six-pack de refresco tiene factor 6; una unidad suelta tiene factor 1. |
| **Producto a granel** | Producto cuya cantidad es decimal y se vende por peso o volumen (arroz, frijol, detergente). |
| **Bodega** | Ubicación de stock no expuesta al cliente: trastienda, almacén, cajas cerradas. |
| **Vitrina** | Ubicación de stock expuesta al cliente y disponible para venta inmediata (estantes, mostrador, refrigerador). |
| **Existencia total** | Bodega + Vitrina. |
| **Reabastecimiento** | Traspaso de cantidad de bodega a vitrina. No modifica la existencia total. |
| **Compra / Entrada** | Ingreso de mercancía al inventario con un costo asociado. |
| **Merma** | Salida de inventario que no es una venta: producto vencido, roto, robado, consumo propio. |
| **Ajuste** | Corrección de la existencia registrada para que coincida con la existencia física real (resultado de un conteo). |
| **Costo unitario** | Lo que le cuesta al negocio una unidad del producto. En Kilo12 siempre es el costo promedio ponderado vigente, derivado como `valor total del inventario ÷ cantidad total`. |
| **Costo promedio ponderado** | Método de valoración en el que, al ingresar mercancía, el costo unitario se recalcula mezclando el valor de la existencia previa con el valor de la entrada, pesado por sus cantidades respectivas. |
| **Valor total del inventario** | Dinero acumulado invertido en la existencia actual de un producto. Junto con la cantidad total, constituye la fuente de verdad del costo (ver RF-COS-02). |
| **Costo de lo vendido** | Suma de los costos unitarios congelados en las líneas de venta de un periodo. Es el sustraendo de la ganancia bruta. |
| **Precio de venta** | Lo que paga el cliente por una unidad. |
| **Margen bruto** | (Precio de venta − Costo unitario) / Precio de venta, expresado en porcentaje. |
| **Ganancia bruta** | Precio de venta − Costo unitario, en dinero. |
| **Sesión de caja** | Periodo de operación entre la apertura y el cierre de caja, normalmente un día. |
| **Arqueo** | Comparación entre el efectivo que el sistema calcula y el efectivo que el usuario cuenta físicamente. |
| **Operador de caja** | Persona que atendió la caja durante una sesión. Se registra como texto al abrirla; no es una cuenta de usuario del sistema. |
| **Comisión** | Retribución del operador de caja, calculada como un porcentaje de la venta de la sesión. Es un cálculo informativo: no mueve dinero por sí solo. |
| **Base de comisión** | Importe sobre el que se aplica el porcentaje: la venta total de la sesión, ya descontadas las devoluciones. No intervienen costos ni mermas. |
| **Ganancia neta de comisión** | Ganancia bruta de la sesión menos la comisión del operador. Es lo que finalmente queda para el local. No contempla otros gastos del negocio. |

---

## 5. Actores

| Actor | Descripción |
|---|---|
| **Dueño / Operador** | Único usuario humano. Registra ventas, administra el catálogo, recibe mercancía, consulta estadísticas y hace el cierre de caja. |
| **Sistema** | Actor no humano responsable de cálculos automáticos, alertas y validaciones. |

---

## 6. Requerimientos funcionales

**Prioridad (MoSCoW):** `M` = Debe (v1 no existe sin esto) · `S` = Debería (v1 queda coja sin esto) · `C` = Podría (deseable, postergable) · `W` = No ahora.

---

### 6.1 Módulo: Catálogo de productos (RF-CAT)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-CAT-01 | El sistema debe permitir registrar un producto con: nombre, SKU, categoría, unidad base, costo unitario actual y stock mínimo. El precio de venta no es un atributo del producto: reside en sus presentaciones (RF-PRS-02). | M |
| RF-CAT-02 | El sistema debe garantizar que el SKU sea único. Si el usuario no proporciona uno, el sistema debe generarlo automáticamente. | M |
| RF-CAT-03 | El sistema debe permitir marcar un producto como **granel**, habilitando cantidades decimales en todas las operaciones de ese producto. | M |
| RF-CAT-04 | El sistema debe permitir definir la unidad base del producto entre: unidad, kg, **lb**, g, L, ml. Una vez que el producto registre movimientos, la unidad base no debe poder modificarse. | M |
| RF-CAT-05 | El sistema debe permitir editar cualquier dato del producto. Los cambios de costo y precio deben quedar registrados en un historial (ver RF-PRE-04). | M |
| RF-CAT-06 | El sistema debe permitir **desactivar** un producto en lugar de eliminarlo, de modo que no aparezca en ventas nuevas pero su historial se conserve. | M |
| RF-CAT-07 | El sistema debe impedir la eliminación definitiva de un producto que tenga movimientos de inventario o ventas asociadas. | M |
| RF-CAT-08 | El sistema debe permitir organizar productos en categorías administrables por el usuario (crear, renombrar, desactivar). | S |
| RF-CAT-09 | El sistema debe permitir buscar productos por coincidencia parcial de nombre, SKU o categoría, sin distinguir mayúsculas ni acentos. | M |
| RF-CAT-10 | El sistema debe permitir asociar una imagen local al producto para facilitar su identificación visual. | C |
| RF-CAT-11 | El sistema debe permitir importar y exportar el catálogo en formato CSV. | S |
| RF-CAT-12 | El sistema debe permitir registrar un código de barras por **presentación** como dato, aun cuando no exista lector conectado, dado que el paquete y la unidad suelta tienen códigos distintos. | C |

**Criterios de aceptación destacados**

- RF-CAT-03: al registrar movimiento de un producto a granel, el sistema acepta `1.250` kg; al hacerlo de un producto por unidad, rechaza `1.5` y exige un entero.
- RF-CAT-09: buscar `"azucar"` encuentra `"Azúcar morena"`.

---

### 6.2 Módulo: Unidades y presentaciones (RF-PRS)

Un mismo producto se vende suelto y en paquete, a precios que **no son proporcionales** entre sí. Este módulo resuelve esa realidad sin duplicar el inventario.

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-PRS-01 | Cada producto debe tener exactamente una **unidad base**, en la cual se registran su existencia y su costo. Toda cantidad de inventario del sistema está expresada en la unidad base del producto. | M |
| RF-PRS-02 | El sistema debe permitir definir una o más **presentaciones de venta** por producto, cada una con: nombre, factor de conversión a la unidad base y precio de venta propio. | M |
| RF-PRS-03 | Todo producto debe tener al menos una presentación. Al crear un producto, el sistema debe generar automáticamente una presentación con factor 1. | M |
| RF-PRS-04 | El precio de cada presentación debe ser **independiente**: no se deriva del precio de ninguna otra presentación ni del factor de conversión. | M |
| RF-PRS-05 | El sistema no debe llevar existencia separada por presentación. La existencia es única por producto y ubicación, expresada en unidad base. | M |
| RF-PRS-06 | Al confirmar la venta de una presentación, el sistema debe descontar de la existencia `cantidad × factor de conversión`, en unidad base. | M |
| RF-PRS-07 | El sistema debe validar la disponibilidad en unidad base antes de permitir la venta de una presentación, e informar cuántas unidades base faltan cuando no alcance. | M |
| RF-PRS-08 | El sistema debe permitir marcar una presentación como **predeterminada**, de modo que la venta rápida la use sin selección adicional. | S |
| RF-PRS-09 | El sistema debe permitir registrar compras en una presentación distinta de la de venta, normalizando cantidad y costo a la unidad base antes de aplicar la valoración (ver RF-COS-13). | M |
| RF-PRS-10 | El sistema debe mostrar, para cada presentación, el **precio equivalente por unidad base**, permitiendo comparar presentaciones entre sí. | S |
| RF-PRS-11 | El sistema debe calcular y mostrar la ganancia bruta y el margen de **cada presentación por separado**, tomando como costo `factor × costo unitario base`. | M |
| RF-PRS-12 | El sistema debe advertir cuando el precio equivalente por unidad base de una presentación mayor resulte superior al de una presentación menor, por tratarse de una anomalía comercial. | C |
| RF-PRS-13 | El sistema debe registrar en cada línea de venta la presentación utilizada, junto con su factor de conversión y su precio vigentes al momento de la venta. | M |
| RF-PRS-14 | El sistema debe validar que el factor de conversión sea mayor que cero. En productos cuya unidad base sea `unidad`, el factor debe ser un número entero. | M |
| RF-PRS-15 | El sistema debe permitir desactivar una presentación sin afectar el historial de ventas que la utilizaron, e impedir su eliminación definitiva si tiene ventas asociadas. | S |

**Ejemplo de validación (debe usarse como caso de prueba)**

Producto: Refresco 500 ml · Unidad base: `unidad` · Costo promedio ponderado: **$41.67 / unidad**

| Presentación | Factor | Precio venta | Costo (factor × $41.67) | Ganancia | Margen | Precio equiv. / unidad |
|---|---|---|---|---|---|---|
| Unidad suelta | 1 | $80.00 | $41.67 | $38.33 | 47.9 % | $80.00 |
| Six-pack | 6 | $300.00 | $250.00 | $50.00 | 16.7 % | $50.00 |

Vender **1 six-pack** descuenta **6 unidades** de la vitrina. Vender **2 unidades sueltas** descuenta **2 unidades**. La existencia es una sola.

**Justificación del diseño**

La alternativa —crear "Refresco suelto" y "Refresco six-pack" como dos productos distintos— produce dos existencias que se contradicen: el sistema cree tener 10 six-packs y 4 sueltos cuando físicamente hay 64 refrescos. Al abrir un paquete para vender suelto, habría que registrar una conversión manual entre productos, y nadie la registra. Con este diseño, **desempacar un six-pack no es una operación de inventario**: los 6 refrescos ya estaban contados.

Como efecto secundario valioso, el sistema hace visible algo que el dueño no suele ver: en el ejemplo, el six-pack deja **menos ganancia por refresco** ($8.33) que la venta suelta ($38.33). Eso es información de negocio, no un detalle contable.

---

### 6.3 Módulo: Inventario y ubicaciones (RF-INV)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-INV-01 | El sistema debe mantener la existencia de cada producto separada en dos ubicaciones: **bodega** y **vitrina**. | M |
| RF-INV-02 | El sistema debe calcular y mostrar la **existencia total** como la suma de ambas ubicaciones. | M |
| RF-INV-03 | El sistema debe registrar todo cambio de existencia como un **movimiento de inventario** inmutable, con: fecha y hora, producto, tipo de movimiento, ubicación origen, ubicación destino, cantidad, costo unitario del momento y motivo. | M |
| RF-INV-04 | Los tipos de movimiento soportados deben ser: `ENTRADA` (compra), `VENTA`, `TRASPASO` (bodega→vitrina o vitrina→bodega), `MERMA`, `AJUSTE`, `DEVOLUCION`. | M |
| RF-INV-05 | El sistema debe permitir consultar el **kárdex** de un producto: la lista cronológica de todos sus movimientos con la existencia resultante después de cada uno. | S |
| RF-INV-06 | El sistema no debe permitir que ninguna ubicación quede con existencia negativa. | M |
| RF-INV-07 | El sistema debe permitir registrar un **conteo físico** de una ubicación, comparando la existencia contada contra la registrada y generando automáticamente los movimientos de `AJUSTE` por la diferencia. | S |
| RF-INV-08 | El sistema debe exigir un motivo obligatorio para todo movimiento de tipo `AJUSTE` y `MERMA`. | M |
| RF-INV-09 | El sistema debe permitir consultar la existencia actual de todos los productos, filtrando por categoría, ubicación y estado de stock (normal, bajo, agotado). | M |
| RF-INV-10 | El sistema debe calcular el **valor del inventario** (existencia × costo unitario) por ubicación y total. | S |

---

### 6.4 Módulo: Vitrina y reabastecimiento (RF-VIT)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-VIT-01 | El sistema debe permitir traspasar una cantidad de un producto de bodega a vitrina, generando un movimiento de tipo `TRASPASO` sin alterar la existencia total. | M |
| RF-VIT-02 | El sistema debe permitir el traspaso inverso (vitrina → bodega) para retirar producto de exhibición. | S |
| RF-VIT-03 | El sistema debe permitir definir por producto una **cantidad objetivo en vitrina** (nivel de exhibición deseado). | S |
| RF-VIT-04 | El sistema debe generar una **lista de reabastecimiento** con los productos cuya existencia en vitrina esté por debajo de su cantidad objetivo y que tengan existencia disponible en bodega. | S |
| RF-VIT-05 | El sistema debe permitir ejecutar el reabastecimiento sugerido de forma masiva, con posibilidad de ajustar cantidad producto por producto antes de confirmar. | C |
| RF-VIT-06 | El sistema debe alertar cuando un producto tenga existencia en bodega pero cero en vitrina (producto disponible pero no exhibido). | S |

**Justificación:** este módulo es la diferencia entre un inventario contable y un inventario operativo. Existencia total alta con vitrina vacía es venta perdida, y el sistema debe hacerla visible.

---

### 6.5 Módulo: Compras y entradas de mercancía (RF-COM)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-COM-01 | El sistema debe permitir registrar una compra con: fecha, proveedor (texto libre o de lista), número de documento opcional y una o más líneas de producto. | M |
| RF-COM-02 | Cada línea de compra debe registrar: producto, presentación de compra, cantidad, costo por presentación y ubicación de destino (bodega por omisión). | M |
| RF-COM-03 | Al confirmar una compra, el sistema debe generar automáticamente los movimientos de `ENTRADA` correspondientes y actualizar la existencia. | M |
| RF-COM-04 | El sistema debe recalcular el **costo promedio ponderado** del producto al registrar cada entrada, conforme a las reglas de §6.6. | M |
| RF-COM-05 | El sistema debe alertar al usuario cuando el costo resultante por unidad base sea mayor o igual al precio equivalente por unidad base de alguna presentación activa, advirtiendo margen nulo o negativo e indicando qué presentaciones quedan afectadas. | M |
| RF-COM-06 | El sistema debe permitir registrar costos adicionales de la compra (flete, acarreo) y distribuirlos proporcionalmente entre las líneas para obtener el costo real. | C |
| RF-COM-07 | El sistema debe permitir anular una compra ya registrada, generando los movimientos de reversa correspondientes en lugar de borrar el registro. | S |
| RF-COM-08 | El sistema debe mantener un historial consultable de compras, filtrable por fecha, proveedor y producto. | S |
| RF-COM-09 | El sistema debe mostrar, al registrar una línea de compra, el último costo pagado por ese producto y la variación porcentual respecto al costo actual. | C |

---

### 6.6 Módulo: Valoración del inventario (RF-COS)

Este módulo define cómo el sistema determina cuánto vale lo que tiene y cuánto costó lo que vendió. Es transversal: alimenta a compras, ventas, mermas y a todo informe de ganancia.

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-COS-01 | El sistema debe valorar el inventario mediante **costo promedio ponderado** a nivel de producto. | M |
| RF-COS-02 | El sistema debe mantener por producto dos magnitudes como fuente de verdad: **cantidad total** y **valor total del inventario**. El costo unitario es un valor derivado (`valor total ÷ cantidad total`) y nunca se almacena como fuente de verdad. | M |
| RF-COS-03 | Al registrar una entrada de mercancía, el sistema debe actualizar ambas magnitudes sumando la cantidad recibida y el importe pagado, y derivar de ahí el nuevo costo unitario. | M |
| RF-COS-04 | El costo unitario debe recalcularse **únicamente** ante movimientos que incorporen mercancía con un costo asociado: `ENTRADA` y `DEVOLUCION` a proveedor. Las ventas, traspasos, mermas y ajustes de cantidad **no** modifican el costo unitario. | M |
| RF-COS-05 | El costo promedio ponderado debe llevarse a nivel de **producto sobre la existencia total**, nunca por ubicación. Un traspaso entre bodega y vitrina mueve cantidad, no valor, y no puede alterar el costo unitario. | M |
| RF-COS-06 | Toda salida de inventario (`VENTA`, `MERMA`, `AJUSTE` negativo) debe descontar del valor total el resultado de `cantidad × costo unitario vigente`, dejando el costo unitario sin cambios. | M |
| RF-COS-07 | El sistema debe registrar el costo unitario vigente en cada línea de venta al confirmarla, de modo que la ganancia histórica no se altere ante recálculos posteriores del costo (concuerda con RF-VTA-13). | M |
| RF-COS-08 | Cuando ingrese mercancía y la existencia previa sea cero, el sistema debe tomar el costo de la entrada como nuevo costo unitario, sin arrastrar el costo anterior. | M |
| RF-COS-09 | El sistema debe manejar el cálculo con precisión suficiente para que la reconstrucción del valor total a partir de operaciones sucesivas no acumule error de redondeo. El costo unitario se redondea solo para su presentación en pantalla, nunca para su almacenamiento. | M |
| RF-COS-10 | Ante la anulación de una compra, el sistema debe revertir la cantidad y el valor aportados por esa compra y recalcular el costo unitario resultante. | S |
| RF-COS-11 | El sistema debe permitir consultar el costo unitario vigente de un producto junto con la fecha de la última entrada que lo modificó. | S |
| RF-COS-12 | El sistema debe registrar en el kárdex del producto el costo unitario resultante después de cada movimiento, de modo que la evolución del costo sea auditable. | C |
| RF-COS-13 | Cuando una entrada se registre en una presentación distinta de la unidad base, el sistema debe normalizar antes de promediar: la cantidad se multiplica por el factor de conversión y el costo de la presentación se divide entre él. La normalización debe operar sobre el importe total de la línea, no sobre el costo unitario redondeado. | M |
| RF-COS-14 | La valoración debe ser independiente de las presentaciones: el costo promedio ponderado existe únicamente en la unidad base. El costo de una presentación es siempre un valor derivado (`factor × costo unitario base`) y nunca se almacena. | M |

**Fórmula de referencia (RF-COS-03 y RF-COS-13)**

```
# Normalización previa cuando la compra se registra en una presentación (RF-COS-13)
cantidad_entrada_base = cantidad_comprada × factor_presentacion
importe_entrada       = cantidad_comprada × costo_por_presentacion

# Valoración (RF-COS-03)
valor_total_nuevo     = valor_total_anterior + importe_entrada
cantidad_total_nueva  = cantidad_total_anterior + cantidad_entrada_base
costo_unitario        = valor_total_nuevo ÷ cantidad_total_nueva
```

Nótese que el importe se calcula sobre el total de la línea. Comprar 10 six-packs a $250 aporta $2,500 y 60 unidades base; el costo por unidad nunca se redondea a $41.67 en el camino.

**Ejemplo de validación (debe usarse como caso de prueba)**

| Fecha | Operación | Cantidad | Valor total | Costo unitario |
|---|---|---|---|---|
| 01/03 | Entrada 100 kg @ $10 | 100 kg | $1,000.00 | $10.00 |
| 10/03 | Venta 40 kg | 60 kg | $600.00 | $10.00 (sin cambio) |
| 15/03 | Entrada 100 kg @ $14 | 160 kg | $2,000.00 | **$12.50** |
| 18/03 | Traspaso 20 kg bodega → vitrina | 160 kg | $2,000.00 | $12.50 (sin cambio) |
| 20/03 | Venta 50 kg @ $18 | 110 kg | $1,375.00 | $12.50 (sin cambio) |

La venta del 20/03 debe registrar un costo de lo vendido de $625.00 y una ganancia bruta de $275.00.

---

### 6.7 Módulo: Venta / Punto de venta (RF-VTA)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-VTA-01 | El sistema debe permitir iniciar una venta y agregar líneas buscando productos por nombre, SKU o categoría desde una única caja de búsqueda. | M |
| RF-VTA-02 | La búsqueda en pantalla de venta debe mostrar resultados de forma incremental mientras el usuario escribe y permitir seleccionarlos únicamente con el teclado. | M |
| RF-VTA-03 | El sistema debe permitir seleccionar la presentación de cada línea de venta y capturar su cantidad, aceptando decimales solo en productos a granel. Cuando el producto tenga una sola presentación activa, debe seleccionarse automáticamente. | M |
| RF-VTA-04 | Para productos a granel, el sistema debe permitir capturar la venta **por cantidad** (ej.: 1.5 kg) o **por importe** (ej.: "quiero 50 de arroz"), calculando la otra magnitud automáticamente. | S |
| RF-VTA-05 | El sistema debe mostrar en todo momento el subtotal, los descuentos aplicados y el total de la venta en curso. | M |
| RF-VTA-06 | El sistema debe permitir modificar la cantidad o eliminar una línea antes de confirmar la venta. | M |
| RF-VTA-07 | El sistema debe permitir cancelar la venta completa antes de confirmarla, sin dejar rastro en el inventario. | M |
| RF-VTA-08 | El sistema debe permitir aplicar un descuento por línea o sobre el total de la venta, en monto o en porcentaje. | S |
| RF-VTA-09 | El sistema debe registrar la forma de pago de la venta entre tres opciones fijas: **efectivo en pesos**, **transferencia** y **efectivo en dólares**. | M |
| RF-VTA-10 | Para pagos en efectivo, el sistema debe permitir capturar el monto recibido y calcular el cambio. El cambio se expresa y se entrega **siempre en pesos** (R-11), aunque el cliente haya pagado en dólares. | M |
| RF-VTA-10b | Para pagos en dólares, el sistema debe convertir el importe con la **tasa vigente**, mostrar el equivalente en pesos antes de confirmar y **registrar la tasa aplicada junto con la venta**. Un cambio posterior de la tasa no debe alterar ventas ya confirmadas. | M |
| RF-VTA-11 | Al confirmar la venta, el sistema debe descontar de la **vitrina** el equivalente en unidad base (`cantidad × factor`) y generar los movimientos de tipo `VENTA`. | M |
| RF-VTA-12 | Si la vitrina no tiene existencia suficiente en unidad base, el sistema debe advertirlo y ofrecer al usuario traspasar desde bodega en el mismo flujo, sin abandonar la venta. | S |
| RF-VTA-13 | El sistema debe registrar en cada línea de venta el **costo unitario base vigente al momento de la venta**, junto con el factor y el precio de la presentación utilizada, para que el cálculo histórico de ganancia sea inmune a cambios posteriores de costo, de precio o de presentaciones. | M |
| RF-VTA-14 | El sistema debe permitir dejar una venta **en espera** y retomarla después, para atender a otro cliente sin perder el trabajo. | C |
| RF-VTA-15 | El sistema debe permitir **anular** una venta ya confirmada únicamente si pertenece a la sesión de caja vigente, devolviendo la mercancía a vitrina mediante movimientos de reversa y registrando el motivo. Las ventas de sesiones ya cerradas no son anulables bajo ninguna circunstancia. | M |
| RF-VTA-16 | El sistema debe permitir registrar una **devolución parcial o total** de una venta de cualquier fecha, con retorno de la mercancía a vitrina o su registro como merma si el producto no es revendible. La devolución no modifica la venta original ni la sesión de caja en que ocurrió: se registra como operación propia, afectando la sesión de caja vigente. | S |
| RF-VTA-16b | El sistema debe indicar explícitamente al usuario, al intentar anular una venta de una sesión cerrada, que la operación disponible es la devolución, y ofrecer continuar por esa vía. | S |
| RF-VTA-17 | El sistema debe asignar a cada venta un folio consecutivo único e irrepetible. | M |
| RF-VTA-18 | El sistema debe permitir consultar y buscar ventas anteriores por folio, fecha o producto vendido. | S |
| RF-VTA-19 | El sistema debe permitir mostrar en pantalla un resumen imprimible/exportable de la venta (comprobante interno, sin valor fiscal). | C |

**Criterios de aceptación destacados**

- RF-VTA-02: el flujo completo de una venta simple (buscar, agregar, cobrar, confirmar) debe poder ejecutarse sin tocar el ratón.
- RF-VTA-13: si hoy vendo un producto con costo 10 y mañana el costo sube a 14, el informe de ganancia de la venta de hoy sigue calculándose con 10.
- RF-VTA-15: una venta de ayer, con la caja de ayer ya cerrada, no ofrece la acción de anular. El arqueo de una sesión cerrada es inmutable.

---

### 6.8 Módulo: Precios y márgenes (RF-PRE)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-PRE-01 | El sistema debe calcular y mostrar, para cada **presentación**, la ganancia bruta y el margen bruto porcentual a partir del costo unitario base vigente y su precio (ver RF-PRS-11). | M |
| RF-PRE-02 | El sistema debe permitir fijar el precio de una presentación de forma directa o indicando un margen objetivo, calculando el precio resultante sobre `factor × costo unitario base`. | S |
| RF-PRE-03 | El sistema debe permitir aplicar un cambio de precio masivo por categoría o por selección, mediante porcentaje o monto fijo, con vista previa antes de confirmar. El usuario debe poder elegir si el cambio aplica a todas las presentaciones o solo a las de un factor determinado. | C |
| RF-PRE-04 | El sistema debe conservar el historial de cambios de costo del producto y de precio de cada presentación, con fecha y valor anterior. | S |
| RF-PRE-05 | El sistema debe permitir configurar el redondeo del precio de venta (ej.: a múltiplos de 0.50) al calcularlo a partir de un margen. | C |
| RF-PRE-06 | El sistema debe alertar sobre presentaciones cuyo margen actual esté por debajo de un umbral configurable. | S |

---

### 6.9 Módulo: Mermas y ajustes (RF-MER)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-MER-01 | El sistema debe permitir registrar una merma indicando producto, cantidad, ubicación y motivo tipificado: deteriorado, dañado, robo, consumo propio, otro. El registro es reactivo: al no controlarse caducidad (R-7), el sistema no anticipa vencimientos. | M |
| RF-MER-02 | El sistema debe registrar toda merma valorizada al costo unitario vigente (RF-COS-06), para cuantificar la pérdida en dinero. | M |
| RF-MER-03 | El sistema debe ofrecer un informe de mermas por periodo, agrupable por producto, categoría y motivo. | S |
| RF-MER-04 | El sistema debe permitir registrar ajustes de inventario por diferencias de conteo, diferenciándolos de las mermas. | S |

---

### 6.10 Módulo: Caja (RF-CAJ)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-CAJ-01 | El sistema debe permitir abrir una sesión de caja registrando el fondo inicial de efectivo. | M |
| RF-CAJ-02 | El sistema debe asociar toda venta a la sesión de caja abierta al momento de confirmarla. | M |
| RF-CAJ-03 | El sistema debe permitir registrar entradas y salidas de efectivo ajenas a la venta (retiro parcial, pago de gastos, ingreso de cambio), con motivo obligatorio. | S |
| RF-CAJ-04 | El sistema debe calcular en todo momento el efectivo esperado en caja **en pesos**: fondo inicial + ventas cobradas en efectivo en pesos + entradas − salidas. **Las transferencias no intervienen** en este cálculo, por no haber pasado por la caja. | M |
| RF-CAJ-05 | El sistema debe permitir cerrar la sesión capturando el efectivo contado físicamente y mostrar la diferencia (sobrante o faltante). El conteo se captura **por moneda**: pesos y dólares se cuentan y se arquean por separado, sin mezclarse en un único saldo. | M |
| RF-CAJ-05b | El sistema debe ofrecer en el cierre dos formas de presentar los dólares, alternables sin recalcular la sesión: **separado**, mostrando los pesos y los dólares en sus respectivas monedas; y **consolidado**, convirtiendo los dólares con la tasa vigente y sumándolos al total en pesos. | M |
| RF-CAJ-05c | El cierre debe mostrar siempre el importe cobrado por **transferencia de forma desglosada**, y a la vez incluirlo en el total vendido. | M |
| RF-CAJ-06 | El sistema debe impedir registrar ventas si no hay una sesión de caja abierta. | S |
| RF-CAJ-07 | El sistema debe conservar el historial de sesiones de caja cerradas, consultable por fecha, con su detalle de ventas y diferencias. | S |
| RF-CAJ-08 | El sistema debe tratar toda sesión de caja cerrada como **inmutable**: ninguna operación posterior puede modificar sus ventas, sus totales, su arqueo ni su comisión liquidada. Las correcciones posteriores se registran como operaciones nuevas en la sesión vigente (ver RF-VTA-16). | M |
| RF-CAJ-09 | El sistema debe permitir registrar, al abrir la sesión, el nombre del **operador de caja** que la atenderá, seleccionándolo de una lista administrable o escribiéndolo. | S |
| RF-CAJ-10 | El cierre de caja debe presentar, además del arqueo de efectivo, el resumen económico de la sesión: venta total, costo de lo vendido, mermas, ganancia bruta, comisión del operador y ganancia neta de comisión (ver §6.12). | M |

---

### 6.11 Módulo: Divisa y tasa de cambio (RF-DIV)

El negocio cobra en pesos, en transferencia y en dólares, pero lleva sus cuentas en una sola moneda (R-4). Este módulo es el puente.

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-DIV-01 | El sistema debe permitir al propietario configurar la **tasa de cambio**, expresada como pesos que equivalen a un dólar. | M |
| RF-DIV-02 | El sistema debe permitir actualizar la tasa en cualquier momento y debe rechazar valores nulos o negativos. | M |
| RF-DIV-03 | El sistema debe **registrar la tasa aplicada en cada cobro en dólares**, de modo que una actualización posterior no altere ventas ya confirmadas ni cierres ya realizados. | M |
| RF-DIV-04 | El sistema debe conservar el historial de cambios de la tasa, con fecha y valor anterior. | S |
| RF-DIV-05 | El sistema debe mostrar la tasa vigente en la pantalla de venta cuando se elija el pago en dólares, para que el operador pueda verificarla antes de cobrar. | S |
| RF-DIV-06 | El sistema debe advertir cuando se registre un cobro en dólares y la tasa no se haya actualizado en más de N días, con N configurable. | C |
| RF-DIV-07 | Todos los importes convertidos deben expresarse en pesos para efectos de ganancia, comisión, valoración e informes. No existen precios ni costos en dólares. | M |

---

### 6.12 Módulo: Comisión del operador de caja (RF-CMS)

El propietario retribuye a quien atiende la caja con un **porcentaje de la venta del día**. El porcentaje lo fija el propietario y puede cambiarlo cuando quiera. Esa comisión se descuenta de la ganancia del local.

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-CMS-01 | El sistema debe permitir configurar un **porcentaje de comisión** del operador de caja, expresado con hasta dos decimales. | M |
| RF-CMS-02 | El sistema debe permitir al propietario modificar ese porcentaje en cualquier momento. | M |
| RF-CMS-03 | El sistema debe calcular la comisión de una sesión como `base de comisión × porcentaje vigente`, y presentarla en la pantalla de cierre de caja. | M |
| RF-CMS-04 | La **base de comisión** debe ser la **venta total** de la sesión: la suma de los importes cobrados en las ventas confirmadas y no anuladas, descontando las devoluciones registradas en la sesión. No intervienen costos, márgenes ni mermas. | M |
| RF-CMS-05 | La base de comisión debe incluir las ventas de la sesión **cualquiera que sea su forma de pago**: efectivo en pesos, transferencia y efectivo en dólares. Las ventas cobradas en dólares se computan por su equivalente en pesos, con la tasa registrada en cada venta (RF-DIV-03). | M |
| RF-CMS-06 | El sistema debe grabar en el cierre de la sesión el porcentaje aplicado, la base de comisión y el importe resultante. Una vez cerrada la sesión, esos tres valores son **inmutables**: un cambio posterior del porcentaje no altera sesiones ya cerradas. | M |
| RF-CMS-07 | El sistema debe **descontar la comisión de la ganancia bruta** de la sesión y presentar el resultado como **ganancia neta de comisión**. | M |
| RF-CMS-08 | Si la base de comisión resulta menor o igual a cero (las devoluciones superaron a las ventas), la comisión debe ser cero. El sistema nunca debe calcular una comisión negativa. | M |
| RF-CMS-09 | El cálculo de la comisión **no debe afectar el arqueo de efectivo**. El efectivo esperado en caja se calcula sin considerarla. | M |
| RF-CMS-10 | Cuando la comisión se pague en efectivo desde la caja, el sistema debe permitir registrarla como una **salida de efectivo** (RF-CAJ-03) con motivo «pago de comisión», dejando constancia de la sesión que la originó. | S |
| RF-CMS-11 | El sistema debe conservar el historial de cambios del porcentaje de comisión, con fecha y valor anterior. | S |
| RF-CMS-12 | El sistema debe ofrecer un informe de comisiones por periodo, agrupado por operador de caja, mostrando base, porcentaje e importe de cada sesión. | S |
| RF-CMS-13 | El sistema debe permitir definir un porcentaje distinto por operador de caja, usando el porcentaje general cuando el operador no tenga uno propio. | C |
| RF-CMS-14 | El sistema debe mostrar en el cierre la venta total que sustenta la base y el porcentaje aplicado, de modo que el operador pueda verificar el importe por sí mismo. | S |

**Ejemplo de validación (debe usarse como caso de prueba)**

Sesión del 12/03 · Operador: Ana · Porcentaje vigente: **2 %**

| Concepto | Importe |
|---|---|
| Venta total de la sesión | $100,000.00 |
| **Base de comisión (venta total)** | **$100,000.00** |
| Comisión al 2 % | **$2,000.00** |
| Costo de lo vendido (costos congelados) | $70,000.00 |
| Ganancia bruta | $30,000.00 |
| **Ganancia neta de comisión** | **$28,000.00** |

Si el 20/03 el propietario sube el porcentaje al 3 %, el cierre del 12/03 sigue mostrando $2,000.00. El nuevo porcentaje rige desde la siguiente sesión que se cierre.

**Justificación del diseño**

Calcular sobre la **venta** hace que el importe sea **verificable por el propio operador**: le basta el total vendido del día para comprobar su pago. Una comisión sobre la ganancia exigiría que conociera los costos de compra de cada producto, es decir, abrirle justamente la información que el riesgo RI-3 recomienda proteger. La regla simple es además la que el operador entiende sin explicaciones.

El porcentaje se congela en el cierre por el mismo motivo por el que el costo se congela en la línea de venta (RF-VTA-13): lo que ya se pagó es un hecho, no un valor que deba recalcularse. Sin RF-CMS-06, subir la comisión en noviembre cambiaría retroactivamente lo que el sistema dice que se debió pagar en octubre, y no habría forma de auditar lo entregado.

Conviene tener presente el efecto de la regla: al no depender del margen, la comisión es la misma vendiendo un producto muy rentable o uno que deja poco. En una sesión de márgenes bajos, la comisión absorbe una porción mayor de la ganancia. El informe de ganancia neta de comisión (RF-CMS-07) es lo que vuelve visible ese efecto.

---

### 6.13 Módulo: Estadísticas e informes (RF-EST)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-EST-01 | El sistema debe ofrecer un panel de inicio con los indicadores del día: número de ventas, total vendido, ganancia bruta estimada, comisión devengada y ticket promedio. | M |
| RF-EST-02 | El sistema debe permitir consultar ventas, ganancia bruta, comisión, ganancia neta de comisión y número de operaciones por periodo (día, semana, mes, rango personalizado). | M |
| RF-EST-03 | El sistema debe ofrecer un informe de **productos más vendidos** por cantidad y por importe, para un periodo dado. Las cantidades deben consolidarse en unidad base para que las ventas sueltas y en paquete sean comparables entre sí. | M |
| RF-EST-03b | El sistema debe permitir desglosar las ventas de un producto **por presentación**, mostrando cuánto se vendió en cada una y qué ganancia aportó, para evaluar la conveniencia de cada formato. | S |
| RF-EST-04 | El sistema debe ofrecer un informe de **productos más rentables**, ordenados por ganancia bruta aportada en el periodo. | S |
| RF-EST-05 | El sistema debe ofrecer un informe de **productos sin movimiento** (sin ventas) en un periodo configurable, señalando el capital inmovilizado en ellos. | S |
| RF-EST-06 | El sistema debe ofrecer un informe de **productos por agotarse**, comparando la existencia actual contra el stock mínimo y contra el ritmo de venta reciente. | M |
| RF-EST-07 | El sistema debe estimar los **días de cobertura** de cada producto (existencia actual ÷ venta promedio diaria del periodo reciente). | C |
| RF-EST-08 | El sistema debe mostrar la evolución de ventas en el tiempo mediante un gráfico de línea o barras por día o mes. | S |
| RF-EST-09 | El sistema debe mostrar la distribución de ventas por categoría en el periodo consultado. | S |
| RF-EST-10 | El sistema debe ofrecer un informe de **valor del inventario** a la fecha, desglosado por ubicación y categoría. | S |
| RF-EST-11 | El sistema debe ofrecer un informe de ventas por hora del día para identificar horarios de mayor actividad. | C |
| RF-EST-11b | El sistema debe ofrecer un informe de ventas **por método de pago** en el periodo consultado, mostrando los dólares tanto en su moneda como en su equivalente en pesos. | S |
| RF-EST-12 | El sistema debe permitir exportar cualquier informe a CSV. | S |
| RF-EST-13 | Todos los informes deben distinguir con claridad los tres niveles, sin confundirlos: **venta** (dinero que entró), **ganancia bruta** (venta − costo de lo vendido) y **ganancia neta de comisión** (ganancia bruta − comisión del operador). | M |

---

### 6.14 Módulo: Datos, respaldo y configuración (RF-DAT)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-DAT-01 | El sistema debe almacenar toda la información en una base de datos local en el equipo del usuario. | M |
| RF-DAT-02 | El sistema debe permitir generar un **respaldo completo** de la base de datos en un archivo, en una ubicación elegida por el usuario. | M |
| RF-DAT-03 | El sistema debe permitir **restaurar** la base de datos desde un archivo de respaldo, advirtiendo explícitamente que se reemplazan los datos actuales. | M |
| RF-DAT-04 | El sistema debe generar respaldos automáticos según una periodicidad configurable y conservar las últimas N copias. | S |
| RF-DAT-05 | El sistema debe recordar al usuario realizar un respaldo si han pasado más de N días desde el último. | S |
| RF-DAT-06 | El sistema debe permitir configurar los datos del negocio: nombre, cantidad de decimales de la moneda, umbrales de alerta, porcentaje de comisión del operador (RF-CMS-01), **tasa de cambio** (RF-DIV-01), modo de cierre preferido (RF-CAJ-05b) y periodicidad de respaldo. | M |
| RF-DAT-07 | El sistema debe validar la integridad del archivo de respaldo antes de restaurarlo, rechazando archivos corruptos o de versión incompatible. | S |
| RF-DAT-08 | El sistema debe migrar automáticamente el esquema de datos al actualizar a una versión nueva de la aplicación, sin pérdida de información. | M |
| RF-DAT-09 | El sistema debe permitir exportar el histórico completo de ventas y movimientos a CSV como respaldo legible independiente de la aplicación. | C |

**Justificación:** operar offline significa que no hay red de seguridad. RF-DAT-02 a RF-DAT-05 no son comodidades: son lo único que separa al negocio de perder su historia completa cuando falle el disco duro.

---

### 6.15 Módulo: Acceso (RF-SEG)

| ID | Requerimiento | Prio. |
|---|---|---|
| RF-SEG-01 | El sistema debe permitir proteger la apertura de la aplicación con un PIN o contraseña local, configurable y desactivable. | S |
| RF-SEG-02 | El sistema no debe transmitir ningún dato fuera del equipo bajo ninguna circunstancia. | M |
| RF-SEG-03 | El sistema debe registrar una bitácora local de operaciones sensibles: anulaciones, ajustes, cambios de precio y restauraciones de respaldo. | C |

---

## 7. Requerimientos no funcionales relevantes

No son el foco de este documento, pero condicionan decisiones funcionales y se listan para que no se pierdan.

| ID | Requerimiento |
|---|---|
| RNF-1 | La búsqueda de productos en pantalla de venta debe responder en menos de 100 ms con un catálogo de al menos 5 000 productos. |
| RNF-2 | La aplicación debe iniciar y quedar operativa en menos de 3 segundos. |
| RNF-3 | La interfaz debe ser completamente operable por teclado en el flujo de venta. |
| RNF-4 | El diseño debe aislar la captura de productos y la emisión de comprobantes tras una frontera que permita incorporar lector de código de barras e impresora térmica sin rediseñar los módulos de negocio. |
| RNF-5 | Ninguna operación confirmada puede dejar el inventario en estado inconsistente ante un cierre abrupto de la aplicación (atomicidad transaccional). |
| RNF-6 | El importe monetario debe representarse con aritmética decimal exacta, nunca con punto flotante binario (`f32`/`f64`). Aplica tanto a precios como al valor total del inventario que sostiene RF-COS-02. |
| RNF-7 | La aplicación debe funcionar en equipos modestos (4 GB RAM) sin degradación perceptible. |
| RNF-8 | La plataforma objetivo principal es **Windows 10 o superior (64 bits)**, que es donde opera el mercadito. **macOS** es plataforma secundaria: debe compilar y funcionar, pero Windows manda en caso de conflicto. |
| RNF-9 | La aplicación debe verificarse en los dos motores web sobre los que se ejecuta —WebView2 en Windows y WebKit en macOS— antes de cada entrega, por no ser equivalentes en su representación visual. |
| RNF-10 | La instalación no debe requerir que el usuario instale dependencias por separado: el instalador debe resolver todo lo necesario para ejecutar la aplicación. |

---

## 8. Decisiones

### 8.1 Decisiones cerradas

| ID | Decisión | Resolución | Fecha | Justificación |
|---|---|---|---|---|
| **D-1** | Método de valoración del costo del inventario | **Costo promedio ponderado** | 2026-09-22 | Modela la realidad física del negocio: las unidades de un mismo producto se mezclan y son indistinguibles. El estado cabe en dos magnitudes por producto (cantidad y valor), sin necesidad de rastrear lotes. Absorbe la volatilidad de precios de proveedor, a diferencia del último costo, que distorsiona el margen ante una sola compra atípica. Descartado FIFO por depender de lotes, excluidos en D-2. Formalizado en §6.6. |
| **D-2** | Control de lotes y fechas de caducidad | **No se implementa** | 2026-09-22 | El mercadito no comercializa perecederos. La existencia se modela como cantidad por ubicación y todas las unidades de un producto son intercambiables. Formalizado en R-7. |
| **D-3** | Cantidad de ubicaciones de exhibición | **Una única vitrina** | 2026-09-22 | El sistema reconoce dos ubicaciones fijas: bodega y vitrina. No se modela "ubicación" como entidad configurable. Formalizado en R-10. Ver riesgo RI-1. |
| **D-4** | Presentaciones de un mismo producto | **Sí: presentaciones con precio propio sobre una unidad base única** | 2026-09-22 | El negocio vende el mismo producto suelto y en paquete a precios no proporcionales (unidad $80, six-pack $300). Se modela una unidad base por producto —donde viven existencia y costo— y N presentaciones con factor de conversión y precio independiente. Se descarta crear productos separados por formato, porque genera existencias contradictorias y exige conversiones manuales que nadie registra. Formalizado en R-9 y §6.2. |
| **D-5** | Política de anulación de ventas | **Solo dentro de la sesión de caja vigente** | 2026-09-22 | Una sesión de caja cerrada ya fue arqueada contra efectivo físico; permitir su modificación posterior invalidaría ese arqueo y todo informe histórico derivado. Las correcciones sobre ventas de sesiones cerradas se canalizan por devolución, que afecta la sesión vigente. Formalizado en RF-VTA-15, RF-VTA-16, RF-VTA-16b y RF-CAJ-08. |
| **D-6** | Base de cálculo de la comisión del operador y vigencia del porcentaje | **Porcentaje sobre la venta total de la sesión, congelado al cerrar; se descuenta de la ganancia bruta** | 2026-09-22 | Decisión del propietario. Calcular sobre la venta hace que el importe sea verificable por el propio operador con solo el total vendido del día, sin necesidad de conocer los costos de compra —información que RI-3 recomienda no exponer—. El porcentaje aplicado se graba en el cierre y no se recalcula: lo ya liquidado es un hecho auditable, igual que el costo congelado en la línea de venta. La comisión se resta de la ganancia bruta para obtener la ganancia neta de comisión, pero no toca el arqueo; su pago, si sale de la caja, es una salida de efectivo ordinaria. Formalizado en §6.12. |

### 8.2 Decisiones abiertas

No quedan decisiones abiertas de modelo de datos. Las pendientes corresponden a la fase de diseño técnico (motor de persistencia, arquitectura de la aplicación, tipo decimal a emplear) y se resolverán en el documento de diseño.

### 8.3 Riesgos asumidos

| ID | Riesgo | Origen | Mitigación |
|---|---|---|---|
| **RI-1** | Si en el futuro el mercadito requiere más de una ubicación de exhibición, agregarla implica migrar datos y revisar todo movimiento de inventario. | D-3 | Modelar la ubicación como un valor enumerado aislado tras una frontera de dominio, de modo que ampliarlo no obligue a reescribir las reglas de negocio. Costo de previsión: bajo. |
| **RI-2** | Un factor de conversión mal capturado (ej. 60 en lugar de 6) corrompe silenciosamente la existencia y el costo promedio. | D-4 | RF-PRS-14 valida el factor; RF-PRS-12 advierte anomalías de precio equivalente, que es el síntoma visible de un factor errado. |
| **RI-3** | Al no haber roles, una empleada que atienda la caja ve los costos de compra, los márgenes y las estadísticas completas del negocio. | R-3 y D-6 | Proteger la aplicación con PIN (RF-SEG-01) y que el propietario abra la sesión. Si el propietario necesita ocultarle esa información de forma efectiva, hay que reabrir la decisión de roles y permisos, que hoy está fuera del alcance. |

---

## 9. Trazabilidad con los objetivos

| Objetivo del negocio (§1) | Requerimientos que lo sostienen |
|---|---|
| Qué tengo y dónde está | RF-PRS-01/05/06, RF-INV-01/02/09, RF-VIT-01/04/06 |
| Cuánto cuesta y cuánto gano | RF-COS-01 a RF-COS-14, RF-COM-04, RF-PRS-11, RF-PRE-01/02, RF-VTA-13, RF-CMS-07, RF-EST-04/13 |
| Qué se vende y qué está parado | RF-EST-03/03b/05/06/07 |
| Cuánto entró y si cuadra la caja | RF-CAJ-01 a RF-CAJ-10, RF-DIV-01 a RF-DIV-07, RF-VTA-09/10/10b/15/16, RF-EST-01/11b |
| Cuánto le corresponde a quien atendió | RF-CMS-01 a RF-CMS-14, RF-CAJ-09/10 |
