//! Evolución del esquema de la base de datos.
//!
//! Las migraciones se aplican solas al arrancar y se numeran con
//! `PRAGMA user_version` (DT-10). Cada una va dentro de una transacción: si
//! falla, el esquema queda como estaba y la aplicación se niega a arrancar,
//! en lugar de operar sobre datos a medio migrar.
//!
//! **Una migración publicada no se edita jamás.** Un error se corrige con
//! una migración nueva; cambiar una anterior dejaría inconsistentes las
//! bases que ya la aplicaron.

use rusqlite::Connection;

use crate::error::ResultadoInfra;

/// Migraciones en orden. El índice más uno es su número de versión.
const MIGRACIONES: &[&str] = &[
    // 1 — Catálogo y existencias.
    r#"
    CREATE TABLE categoria (
        id      INTEGER PRIMARY KEY,
        nombre  TEXT    NOT NULL UNIQUE,
        activa  INTEGER NOT NULL DEFAULT 1 CHECK (activa IN (0,1))
    );

    CREATE TABLE producto (
        id               INTEGER PRIMARY KEY,
        sku              TEXT    NOT NULL UNIQUE,
        nombre           TEXT    NOT NULL,
        categoria_id     INTEGER REFERENCES categoria(id),
        unidad_base      TEXT    NOT NULL CHECK (unidad_base IN
                                 ('unidad','kg','lb','g','L','ml')),
        es_granel        INTEGER NOT NULL DEFAULT 0 CHECK (es_granel IN (0,1)),
        -- Valor acumulado invertido, en millonésimas. Junto con la suma de
        -- existencias es la fuente de verdad del costo (RF-COS-02).
        valor_total      INTEGER NOT NULL DEFAULT 0 CHECK (valor_total >= 0),
        stock_minimo     INTEGER NOT NULL DEFAULT 0 CHECK (stock_minimo >= 0),
        objetivo_vitrina INTEGER NOT NULL DEFAULT 0 CHECK (objetivo_vitrina >= 0),
        activo           INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0,1)),
        creado_en        TEXT    NOT NULL
    );

    CREATE TABLE presentacion (
        id                INTEGER PRIMARY KEY,
        producto_id       INTEGER NOT NULL REFERENCES producto(id),
        nombre            TEXT    NOT NULL,
        -- Milésimas: un six-pack es 6000. Debe ser mayor que cero (RF-PRS-14).
        factor            INTEGER NOT NULL CHECK (factor > 0),
        precio            INTEGER NOT NULL CHECK (precio >= 0),
        es_predeterminada INTEGER NOT NULL DEFAULT 0 CHECK (es_predeterminada IN (0,1)),
        codigo_barras     TEXT,
        activa            INTEGER NOT NULL DEFAULT 1 CHECK (activa IN (0,1)),
        UNIQUE (producto_id, nombre)
    );

    CREATE TABLE existencia (
        producto_id INTEGER NOT NULL REFERENCES producto(id),
        ubicacion   TEXT    NOT NULL CHECK (ubicacion IN ('BODEGA','VITRINA')),
        -- Milésimas de la unidad base. Nunca negativa (RF-INV-06).
        cantidad    INTEGER NOT NULL DEFAULT 0 CHECK (cantidad >= 0),
        PRIMARY KEY (producto_id, ubicacion)
    );

    CREATE INDEX idx_producto_nombre    ON producto(nombre);
    CREATE INDEX idx_producto_categoria ON producto(categoria_id);
    CREATE INDEX idx_presentacion_prod  ON presentacion(producto_id);
    "#,
    // 2 — «Se vende en fracciones» deja de ser un dato propio.
    //
    // Se deduce de la unidad base: lo que se cuenta por unidades se vende
    // entero y lo que se pesa o se mide se fracciona (RF-CAT-03). La
    // columna permitía estados que no existen en el mostrador, como «se
    // cuenta en libras pero no se puede vender media libra».
    r#"
    ALTER TABLE producto DROP COLUMN es_granel;
    "#,
    // 3 — El kárdex: todo cambio de existencia deja su asiento (RF-INV-03).
    //
    // La tabla `existencia` guarda el saldo; esta guarda por qué el saldo es
    // el que es. Ninguna fila se edita ni se borra nunca: un error se
    // corrige con otro movimiento que lo compense.
    r#"
    CREATE TABLE movimiento (
        id                 INTEGER PRIMARY KEY,
        producto_id        INTEGER NOT NULL REFERENCES producto(id),
        tipo               TEXT    NOT NULL CHECK (tipo IN
                                   ('ENTRADA','VENTA','TRASPASO','MERMA',
                                    'AJUSTE','DEVOLUCION')),
        -- De dónde salió y a dónde fue. Una entrada no tiene origen y una
        -- merma no tiene destino: la mercancía entra o sale del negocio.
        origen             TEXT    CHECK (origen  IN ('BODEGA','VITRINA')),
        destino            TEXT    CHECK (destino IN ('BODEGA','VITRINA')),
        cantidad           INTEGER NOT NULL CHECK (cantidad > 0),
        -- Costo vigente en el momento, congelado (RF-COS-07). Recalcular el
        -- pasado con el costo de hoy convertiría el historial en ficción.
        costo_unitario     INTEGER NOT NULL CHECK (costo_unitario >= 0),
        -- Existencia que quedó después del movimiento, para que el kárdex se
        -- lea sin recalcular nada (RF-INV-05).
        bodega_resultante  INTEGER NOT NULL CHECK (bodega_resultante  >= 0),
        vitrina_resultante INTEGER NOT NULL CHECK (vitrina_resultante >= 0),
        -- Obligatorio en mermas y ajustes (RF-INV-08).
        motivo             TEXT,
        ocurrido_en        TEXT    NOT NULL
    );

    CREATE INDEX idx_movimiento_producto ON movimiento(producto_id, id);
    "#,
    // 4 — Historial de precios (RF-PRE-04, RF-CAT-05).
    //
    // Mismo motivo que el kárdex: el precio de ayer no se puede deducir del
    // de hoy. Si no se anota cuando cambia, se pierde para siempre, y con
    // él la posibilidad de saber si una venta vieja dejaba ganancia.
    r#"
    CREATE TABLE historial_precio (
        id             INTEGER PRIMARY KEY,
        producto_id    INTEGER NOT NULL REFERENCES producto(id),
        presentacion_id INTEGER NOT NULL REFERENCES presentacion(id),
        anterior       INTEGER NOT NULL CHECK (anterior >= 0),
        nuevo          INTEGER NOT NULL CHECK (nuevo >= 0),
        cambiado_en    TEXT    NOT NULL
    );

    CREATE INDEX idx_historial_precio_producto
        ON historial_precio(producto_id, id);
    "#,
    // 5 — La venta (RF-VTA).
    //
    // Cada línea guarda COPIA del nombre, el factor, el precio y el costo
    // del momento (RF-VTA-13). No son referencias: si mañana sube el costo
    // o se renombra el producto, la venta de hoy debe seguir contando lo
    // que pasó hoy.
    //
    // Los pagos van en su propia tabla porque un cobro puede ser mixto:
    // algo en efectivo, algo por transferencia y algo en dólares.
    r#"
    CREATE TABLE venta (
        id          INTEGER PRIMARY KEY,
        -- Consecutivo e irrepetible (RF-VTA-17).
        folio       INTEGER NOT NULL UNIQUE,
        total       INTEGER NOT NULL CHECK (total >= 0),
        costo_total INTEGER NOT NULL CHECK (costo_total >= 0),
        -- Siempre en pesos, aunque se haya pagado en dólares (R-11).
        vuelto      INTEGER NOT NULL CHECK (vuelto >= 0),
        ocurrido_en TEXT    NOT NULL
    );

    CREATE TABLE venta_linea (
        id                  INTEGER PRIMARY KEY,
        venta_id            INTEGER NOT NULL REFERENCES venta(id),
        producto_id         INTEGER NOT NULL REFERENCES producto(id),
        presentacion_id     INTEGER NOT NULL REFERENCES presentacion(id),
        nombre_producto     TEXT    NOT NULL,
        nombre_presentacion TEXT    NOT NULL,
        cantidad            INTEGER NOT NULL CHECK (cantidad > 0),
        factor              INTEGER NOT NULL CHECK (factor > 0),
        precio              INTEGER NOT NULL CHECK (precio >= 0),
        costo_unitario      INTEGER NOT NULL CHECK (costo_unitario >= 0)
    );

    CREATE TABLE venta_pago (
        id              INTEGER PRIMARY KEY,
        venta_id        INTEGER NOT NULL REFERENCES venta(id),
        metodo          TEXT    NOT NULL CHECK (metodo IN
                                ('EFECTIVO_CUP','TRANSFERENCIA','EFECTIVO_USD')),
        entregado       INTEGER NOT NULL CHECK (entregado >= 0),
        -- Tasa aplicada, congelada. Solo en los pagos en dólares.
        tasa            INTEGER,
        equivalente_cup INTEGER NOT NULL CHECK (equivalente_cup >= 0)
    );

    -- Ajustes del negocio que el dueño puede cambiar, como la tasa vigente.
    CREATE TABLE configuracion (
        clave TEXT PRIMARY KEY,
        valor TEXT NOT NULL
    );

    CREATE INDEX idx_venta_linea_venta ON venta_linea(venta_id);
    CREATE INDEX idx_venta_pago_venta  ON venta_pago(venta_id);
    "#,
    // 6 — La sesión de caja (RF-CAJ).
    //
    // El cierre guarda sus cifras CONGELADAS en vez de recalcularlas al
    // leerlas. No es desnormalización por comodidad: una sesión cerrada ya
    // se arqueó contra dinero físico (RF-CAJ-08), y si sus totales
    // dependieran del estado de hoy, ese arqueo dejaría de significar nada.
    // Lo mismo vale para el porcentaje de comisión (D-6).
    //
    // `venta.sesion_id` admite nulos por las ventas anteriores a este
    // cambio: no pertenecen a ninguna sesión y no hay forma honesta de
    // inventarles una.
    r#"
    CREATE TABLE sesion_caja (
        id             INTEGER PRIMARY KEY,
        -- Quién atiende la caja. Es un dato de la sesión, no una cuenta
        -- de usuario (R-3, RF-CAJ-09).
        operador       TEXT    NOT NULL,
        fondo_inicial  INTEGER NOT NULL CHECK (fondo_inicial >= 0),
        estado         TEXT    NOT NULL CHECK (estado IN ('ABIERTA','CERRADA')),
        abierta_en     TEXT    NOT NULL,
        cerrada_en     TEXT,

        -- Todo lo que sigue se escribe UNA vez, al cerrar, y no se vuelve
        -- a tocar.
        modo_cierre    TEXT    CHECK (modo_cierre IN ('SEPARADO','CONSOLIDADO')),
        contado_cup    INTEGER,
        contado_usd    INTEGER,
        esperado_cup   INTEGER,
        esperado_usd   INTEGER,
        vendido_cup       INTEGER,
        vendido_transferencia INTEGER,
        vendido_usd       INTEGER,
        vendido_usd_en_cup INTEGER,
        costo_vendido  INTEGER,
        merma_costo    INTEGER,
        comision_porcentaje INTEGER,
        comision_base  INTEGER,
        comision_importe INTEGER
    );

    -- Una sola sesión abierta a la vez: dos turnos compartiendo la gaveta
    -- hacen imposible imputar un faltante. El índice parcial lo impone la
    -- base, no un `if` que alguien puede olvidarse de escribir.
    CREATE UNIQUE INDEX idx_sesion_unica_abierta
        ON sesion_caja(estado) WHERE estado = 'ABIERTA';

    CREATE TABLE movimiento_efectivo (
        id          INTEGER PRIMARY KEY,
        sesion_id   INTEGER NOT NULL REFERENCES sesion_caja(id),
        tipo        TEXT    NOT NULL CHECK (tipo IN ('ENTRADA','SALIDA')),
        importe     INTEGER NOT NULL CHECK (importe > 0),
        -- Obligatorio: un retiro sin explicación es indistinguible de un
        -- faltante (RF-CAJ-03).
        motivo      TEXT    NOT NULL,
        ocurrido_en TEXT    NOT NULL
    );

    CREATE INDEX idx_movimiento_efectivo_sesion
        ON movimiento_efectivo(sesion_id, id);

    ALTER TABLE venta ADD COLUMN sesion_id INTEGER REFERENCES sesion_caja(id);
    ALTER TABLE venta ADD COLUMN anulada_en TEXT;
    ALTER TABLE venta ADD COLUMN motivo_anulacion TEXT;

    CREATE INDEX idx_venta_sesion ON venta(sesion_id);
    "#,
    // 7 — La venta en espera (RF-VTA-14).
    //
    // Guarda solo la INTENCIÓN: producto, presentación y cantidad. Ni
    // precio, ni costo, ni tasa, ni folio, ni sesión: todo eso se decide al
    // cobrar, porque una venta en espera todavía no ocurrió. Por eso vive en
    // tablas propias y no como una fila de `venta` con otro estado: el folio
    // consecutivo, los totales obligatorios y cada consulta de caja e
    // informes tendrían que aprender a ignorarla, y basta con que una lo
    // olvide para descuadrar un arqueo.
    //
    // Los renglones caen con su cabecera (ON DELETE CASCADE): cobrar o
    // descartar una espera es borrar una fila, no acordarse de dos.
    r#"
    CREATE TABLE venta_en_espera (
        id        INTEGER PRIMARY KEY,
        -- Texto libre para reconocerla en la lista. Recortado; nunca vacío.
        nota      TEXT    CHECK (nota IS NULL OR length(nota) BETWEEN 1 AND 120),
        creada_en TEXT    NOT NULL
    );

    CREATE TABLE venta_en_espera_linea (
        id              INTEGER PRIMARY KEY,
        espera_id       INTEGER NOT NULL
                        REFERENCES venta_en_espera(id) ON DELETE CASCADE,
        producto_id     INTEGER NOT NULL REFERENCES producto(id),
        presentacion_id INTEGER NOT NULL REFERENCES presentacion(id),
        -- Milésimas de presentaciones, como `venta_linea.cantidad`.
        cantidad        INTEGER NOT NULL CHECK (cantidad > 0),
        -- Posición en la venta: se retoma en el orden en que se armó.
        orden           INTEGER NOT NULL CHECK (orden >= 0),
        UNIQUE (espera_id, orden)
    );
    "#,
    // 8 — Historial de revaluaciones de inventario.
    //
    // El costo no se almacena: se deriva de `producto.valor_total` entre la
    // existencia (RF-COS-02). Así que fijar un costo a mano es mover el
    // valor del inventario, y eso crea o destruye valor que nadie compró.
    //
    // Va en su propia tabla y NO en el kárdex, por dos razones. La de
    // diseño: el kárdex explica saldos de existencia, y una revaluación no
    // mueve ni una unidad. La de esquema: `movimiento.cantidad` exige
    // `> 0`, así que un asiento sin cantidad no cabe.
    //
    // Es el mismo motivo por el que `historial_precio` existe aparte: el
    // costo de ayer no se deduce del de hoy, y si no se anota al cambiarlo
    // se pierde para siempre.
    //
    // Se guardan las dos cifras de las dos puntas —costo y valor, antes y
    // después— y la existencia sobre la que se aplicó. Ninguna de las tres
    // se puede reconstruir después: la existencia se mueve con cada venta.
    r#"
    CREATE TABLE historial_costo (
        id             INTEGER PRIMARY KEY,
        producto_id    INTEGER NOT NULL REFERENCES producto(id),
        -- Costo por unidad base, en millonésimas.
        anterior       INTEGER NOT NULL CHECK (anterior >= 0),
        nuevo          INTEGER NOT NULL CHECK (nuevo >= 0),
        -- Existencia total en el momento de revalorar, en milésimas.
        -- Mayor que cero: sin existencia no hay costo que fijar.
        existencia     INTEGER NOT NULL CHECK (existencia > 0),
        valor_anterior INTEGER NOT NULL CHECK (valor_anterior >= 0),
        valor_nuevo    INTEGER NOT NULL CHECK (valor_nuevo >= 0),
        cambiado_en    TEXT    NOT NULL
    );

    CREATE INDEX idx_historial_costo_producto
        ON historial_costo(producto_id, id);
    "#,
    // 9 — El consumo de los dueños (RF-CON).
    //
    // Los dueños se llevan mercancía para su casa. No es una venta —no
    // entra dinero— ni una merma —no se perdió nada—: el valor sale del
    // inventario y se convierte en una deuda al costo contra la cuenta que
    // lo cargó. El negocio no es más pobre, tiene otra cosa.
    //
    // ## Por qué esta migración reconstruye `movimiento`
    //
    // Es la PRIMERA reconstrucción de tabla del proyecto, y toca la más
    // delicada: el kárdex. Hace falta porque un consumo es una salida de
    // existencia y tiene que dejar asiento (RF-INV-03), lo que exige dos
    // valores nuevos en `movimiento.tipo`. SQLite no permite modificar un
    // `CHECK`, así que la tabla se rehace.
    //
    // No se cuela como `MERMA` a propósito: la merma del turno se calcula
    // filtrando `tipo = 'MERMA'` por ventana de tiempo, de modo que cada
    // consumo falsearía el arqueo diciendo que el local perdió mercancía
    // que en realidad tiene apuntada como deuda.
    //
    // Los DOS valores entran de golpe —`CONSUMO` y `CONSUMO_ANULADO`—
    // porque una segunda reconstrucción costaría lo mismo que esta.
    //
    // `movimiento` es tabla hija: su única clave foránea es de salida
    // (`producto_id`) y ninguna otra tabla la referencia, así que no hay
    // que desactivar claves foráneas —cosa que además sería imposible,
    // porque `PRAGMA foreign_keys` no hace nada dentro de una transacción
    // y cada migración corre dentro de una—.
    //
    // El `id` se copia EXPLÍCITAMENTE: es alias de `rowid` sin
    // `AUTOINCREMENT`, y el kárdex se lee con `ORDER BY id DESC`. Dejar que
    // se reasignen cambiaría el orden del historial.
    r#"
    CREATE TABLE movimiento_nuevo (
        id                 INTEGER PRIMARY KEY,
        producto_id        INTEGER NOT NULL REFERENCES producto(id),
        tipo               TEXT    NOT NULL CHECK (tipo IN
                                   ('ENTRADA','VENTA','TRASPASO','MERMA',
                                    'AJUSTE','DEVOLUCION','CONSUMO',
                                    'CONSUMO_ANULADO')),
        origen             TEXT    CHECK (origen  IN ('BODEGA','VITRINA')),
        destino            TEXT    CHECK (destino IN ('BODEGA','VITRINA')),
        cantidad           INTEGER NOT NULL CHECK (cantidad > 0),
        costo_unitario     INTEGER NOT NULL CHECK (costo_unitario >= 0),
        bodega_resultante  INTEGER NOT NULL CHECK (bodega_resultante  >= 0),
        vitrina_resultante INTEGER NOT NULL CHECK (vitrina_resultante >= 0),
        motivo             TEXT,
        ocurrido_en        TEXT    NOT NULL
    );

    INSERT INTO movimiento_nuevo
        (id, producto_id, tipo, origen, destino, cantidad, costo_unitario,
         bodega_resultante, vitrina_resultante, motivo, ocurrido_en)
    SELECT id, producto_id, tipo, origen, destino, cantidad, costo_unitario,
           bodega_resultante, vitrina_resultante, motivo, ocurrido_en
      FROM movimiento;

    DROP TABLE movimiento;
    ALTER TABLE movimiento_nuevo RENAME TO movimiento;

    CREATE INDEX idx_movimiento_producto ON movimiento(producto_id, id);

    -- Las cuentas de consumo. Es una tabla y no un `CHECK` con tres
    -- nombres: añadir o retirar una persona no puede exigir una migración,
    -- porque las personas no son esquema. Una cuenta con historial no se
    -- borra, se desactiva, igual que un producto.
    CREATE TABLE cuenta_consumo (
        id     INTEGER PRIMARY KEY,
        nombre TEXT    NOT NULL UNIQUE,
        activa INTEGER NOT NULL DEFAULT 1 CHECK (activa IN (0,1))
    );

    INSERT INTO cuenta_consumo (nombre) VALUES ('Ivett'), ('Claudia'), ('Inés');

    -- La cabecera del consumo.
    --
    -- `tomado_por` es texto libre y no una referencia a la cuenta: quien se
    -- lleva la mercancía puede no ser la dueña de la cuenta que la paga. Es
    -- un dato, no una credencial, igual que `sesion_caja.operador` (R-3).
    --
    -- `total_costo` se guarda en vez de derivarse. No es comodidad: es lo
    -- que permite que el saldo de una cuenta sea una suma de dos columnas
    -- enteras en SQL. Derivarlo exigiría multiplicar cantidad por factor
    -- por costo, y esa aritmética no se hace en SQL en este proyecto.
    -- Mismo criterio que `venta.costo_total`.
    CREATE TABLE consumo (
        id               INTEGER PRIMARY KEY,
        cuenta_id        INTEGER NOT NULL REFERENCES cuenta_consumo(id),
        tomado_por       TEXT    NOT NULL,
        -- Costo total congelado, en millonésimas.
        total_costo      INTEGER NOT NULL CHECK (total_costo >= 0),
        ocurrido_en      TEXT    NOT NULL,
        -- Un consumo anulado deja de contar para el saldo y se queda por su
        -- rastro, igual que una venta anulada (RF-VTA-15).
        anulado_en       TEXT,
        motivo_anulacion TEXT
    );

    -- Los renglones, con todo congelado como en `venta_linea` (RF-VTA-13):
    -- si mañana sube el costo del arroz, la deuda que se acordó hoy sigue
    -- diciendo lo que decía hoy.
    --
    -- `origen` va por LÍNEA y no por consumo: se puede coger el aceite de
    -- la vitrina y el arroz de la bodega en el mismo acto.
    CREATE TABLE consumo_linea (
        id                  INTEGER PRIMARY KEY,
        consumo_id          INTEGER NOT NULL REFERENCES consumo(id),
        producto_id         INTEGER NOT NULL REFERENCES producto(id),
        presentacion_id     INTEGER NOT NULL REFERENCES presentacion(id),
        nombre_producto     TEXT    NOT NULL,
        nombre_presentacion TEXT    NOT NULL,
        origen              TEXT    NOT NULL CHECK (origen IN ('BODEGA','VITRINA')),
        -- Milésimas de presentaciones, como `venta_linea.cantidad`.
        cantidad            INTEGER NOT NULL CHECK (cantidad > 0),
        -- Unidades base que lleva cada presentación.
        factor              INTEGER NOT NULL CHECK (factor > 0),
        costo_unitario      INTEGER NOT NULL CHECK (costo_unitario >= 0)
    );

    -- Las liquidaciones.
    --
    -- NO llevan sesión de caja a propósito: liquidar no es un ingreso. El
    -- negocio ya gastó ese dinero cuando compró la mercancía, así que el
    -- pago recupera un costo y no gana nada. Meterlo en las cifras de venta
    -- del turno sería inflar ingresos que no existen. Si alguna vez el
    -- dinero entra físicamente a la gaveta, se anota a mano como entrada de
    -- efectivo (RF-CAJ-03).
    --
    -- Un pago anulado sube el saldo otra vez: es la otra dirección del
    -- mismo eje que un consumo anulado, que lo baja.
    CREATE TABLE pago_consumo (
        id               INTEGER PRIMARY KEY,
        cuenta_id        INTEGER NOT NULL REFERENCES cuenta_consumo(id),
        importe          INTEGER NOT NULL CHECK (importe > 0),
        nota             TEXT,
        ocurrido_en      TEXT    NOT NULL,
        anulado_en       TEXT,
        motivo_anulacion TEXT
    );

    CREATE INDEX idx_consumo_cuenta       ON consumo(cuenta_id, id);
    CREATE INDEX idx_consumo_linea_cabeza ON consumo_linea(consumo_id);
    CREATE INDEX idx_consumo_linea_prod   ON consumo_linea(producto_id);
    CREATE INDEX idx_pago_consumo_cuenta  ON pago_consumo(cuenta_id, id);
    "#,
];

/// Lleva el esquema a la última versión.
pub fn aplicar(conexion: &Connection) -> ResultadoInfra<()> {
    aplicar_hasta(conexion, version_esperada())
}

/// Aplica las migraciones pendientes hasta la versión indicada.
///
/// Existe para **reproducir una versión anterior del esquema**, que es lo
/// que hace falta para probar una actualización: hay que partir de la base
/// que tiene instalada quien va a actualizar, y la única forma honesta de
/// construirla es aplicar las migraciones que existían entonces y parar.
/// Vale porque una migración publicada no se edita jamás, así que las de
/// hoy son byte a byte las de entonces.
///
/// La aplicación nunca llama a esto con un tope: arrancar con el esquema a
/// medio migrar dejaría a la mitad del código hablando de tablas que no
/// existen. Para eso está [`aplicar`].
pub fn aplicar_hasta(conexion: &Connection, hasta: usize) -> ResultadoInfra<()> {
    let version: i64 = conexion.query_row("PRAGMA user_version", [], |fila| fila.get(0))?;
    let version = usize::try_from(version).unwrap_or(0);

    for (indice, migracion) in MIGRACIONES
        .iter()
        .enumerate()
        .take(hasta.min(MIGRACIONES.len()))
        .skip(version)
    {
        conexion.execute_batch(&format!(
            "BEGIN;\n{}\nPRAGMA user_version = {};\nCOMMIT;",
            migracion,
            indice.saturating_add(1)
        ))?;
    }

    Ok(())
}

/// Versión de esquema que esta compilación espera.
///
/// Sirve para rechazar un respaldo creado por una versión posterior
/// (RF-DAT-07).
pub fn version_esperada() -> usize {
    MIGRACIONES.len()
}
