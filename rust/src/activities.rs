//! Cosmetic, timed prop activities built from the supplied art pack.
//!
//! The source artwork is converted at development time to compact integer
//! rectangle records in `assets/activity-scenes.bin`; no SVG or PNG decoding
//! occurs in the running application. The imported mascot bodies and faces
//! are removed during import. This module only draws accessories and effects.

use crate::render::{Canvas, FEET_X, FEET_Y};
use crate::Vec2;

const SCENE_DATA: &[u8] = include_bytes!("../assets/activity-scenes.bin");
const SCENE_COUNT: usize = 75;
const GRID_UNIT: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityId(pub u8);

impl ActivityId {
    pub const fn new(index: usize) -> Option<Self> {
        if index < SCENE_COUNT {
            Some(Self(index as u8))
        } else {
            None
        }
    }
    pub const fn index(self) -> usize {
        self.0 as usize
    }
    pub fn spec(self) -> &'static ActivitySpec {
        CATALOG
            .get(self.index())
            .filter(|spec| spec.id == self)
            .unwrap_or(&CATALOG[0])
    }
    pub const fn duration(self) -> f64 {
        if self.index() < SCENE_COUNT {
            CATALOG[self.index()].duration
        } else {
            3.8
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActivityGroup {
    Mood,
    Coding,
    Building,
    Nature,
    Food,
    Celebration,
    Error,
    Security,
    Reflection,
    Strength,
}
impl ActivityGroup {
    pub const fn label_ru(self) -> &'static str {
        match self {
            Self::Mood => "Настроение",
            Self::Coding => "Работа и код",
            Self::Building => "Мастерская",
            Self::Nature => "Природа",
            Self::Food => "Еда",
            Self::Celebration => "Праздник",
            Self::Error => "Забавные неудачи",
            Self::Security => "Приключения",
            Self::Reflection => "Размышления",
            Self::Strength => "Спорт",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivitySpec {
    pub id: ActivityId,
    pub slug: &'static str,
    pub title_ru: &'static str,
    pub group: ActivityGroup,
    /// Short cosmetic bubble text. It describes the illustration, never a
    /// real system scan, network action, or security result.
    pub phrase: &'static str,
    pub duration: f64,
}

macro_rules! entry {
    ($index:literal, $slug:literal, $title:literal, $group:ident, $phrase:literal, $duration:literal) => {
        ActivitySpec {
            id: ActivityId($index),
            slug: $slug,
            title_ru: $title,
            group: ActivityGroup::$group,
            phrase: $phrase,
            duration: $duration,
        }
    };
}

/// Stable catalog order: case-insensitively sorted source SVG filenames.
pub const CATALOG: [ActivitySpec; SCENE_COUNT] = [
    entry!(
        0,
        "angry-at-laptop",
        "Сердитый ноутбук",
        Coding,
        "Печатаю сердитый код!",
        3.8
    ),
    entry!(
        1,
        "angry-pixel-coffee",
        "Крепкий кофе",
        Food,
        "Кофе поможет проснуться!",
        3.5
    ),
    entry!(
        2,
        "angry-powerful-lightning",
        "Молния энергии",
        Mood,
        "Вот это заряд!",
        3.4
    ),
    entry!(
        3,
        "api-success-pixel",
        "Удачный API",
        Coding,
        "Нарисую ответ API!",
        3.7
    ),
    entry!(
        4,
        "artboard-24cla",
        "Золотое торжество",
        Celebration,
        "Устрою маленький праздник!",
        4.1
    ),
    entry!(
        5,
        "artboard-24cla2",
        "Сонная улыбка",
        Mood,
        "Немного отдыха...",
        3.6
    ),
    entry!(
        6,
        "artboard-24cla3",
        "Зелёный щит",
        Security,
        "Держу щит крепко!",
        3.8
    ),
    entry!(
        7,
        "artboard-25cla",
        "Быстрые шаги",
        Strength,
        "Шаг за шагом!",
        3.6
    ),
    entry!(
        8,
        "artboard-25cla2",
        "Пиксельная игра",
        Coding,
        "Поиграю в пиксели!",
        3.9
    ),
    entry!(
        9,
        "artboard-25cla3",
        "Маленький сад",
        Nature,
        "Растёт мой сад!",
        4.0
    ),
    entry!(
        10,
        "artboard-2cla",
        "Зелёная диаграмма",
        Coding,
        "Соберу диаграмму!",
        3.8
    ),
    entry!(
        11,
        "artboard-2cla2",
        "Знак ошибки",
        Error,
        "Ой, пиксельная ошибка!",
        3.5
    ),
    entry!(
        12,
        "artboard-2cla3",
        "Синий костюм",
        Mood,
        "Сегодня новый образ!",
        3.9
    ),
    entry!(
        13,
        "artboard-3cla",
        "Два пузырька",
        Coding,
        "Поговорю с пузырьками!",
        3.8
    ),
    entry!(
        14,
        "artboard-3cla2",
        "Цветные искры",
        Celebration,
        "Искры летят!",
        3.6
    ),
    entry!(
        15,
        "artboard-3cla3",
        "Луч света",
        Reflection,
        "Подумаю над идеей!",
        3.8
    ),
    entry!(
        16,
        "artboard-4cla",
        "Сердце идеи",
        Celebration,
        "Берегу эту идею!",
        4.0
    ),
    entry!(
        17,
        "artboard-4cla2",
        "Пиксельный переполох",
        Mood,
        "Пиксели кружатся!",
        3.7
    ),
    entry!(
        18,
        "artboard-4cla3",
        "Песочные часы",
        Reflection,
        "Подожду ещё миг!",
        3.9
    ),
    entry!(
        19,
        "baking-steaming-pie",
        "Пирог из пикселей",
        Food,
        "Пирог почти готов!",
        4.2
    ),
    entry!(
        20,
        "broken-heart-sad",
        "Грустное сердечко",
        Mood,
        "Обниму сердечко.",
        3.7
    ),
    entry!(
        21,
        "building-colorful-blocks",
        "Цветные кубики",
        Building,
        "Сложу кубик к кубику!",
        4.0
    ),
    entry!(
        22,
        "confusion-spiral-pixel",
        "Спираль мыслей",
        Reflection,
        "Распутаю эту мысль!",
        3.8
    ),
    entry!(
        23,
        "crying-sad-reaction",
        "Капли грусти",
        Mood,
        "Слёзы скоро высохнут.",
        3.8
    ),
    entry!(
        24,
        "data-blocks-pixel",
        "Блоки данных",
        Coding,
        "Разложу блоки по местам!",
        3.7
    ),
    entry!(
        25,
        "debugging-system-bugs",
        "Пиксельная отладка",
        Coding,
        "Ищу жучка в рисунке!",
        4.0
    ),
    entry!(
        26,
        "developer-pair-programming-scene",
        "Парное программирование",
        Coding,
        "Пишем код вместе!",
        4.1
    ),
    entry!(
        27,
        "disconnected-plug-pixel",
        "Вилка и розетка",
        Coding,
        "Проверю соединение на рисунке!",
        3.7
    ),
    entry!(
        28,
        "dizzy-failed-state",
        "Головокружительный сбой",
        Error,
        "Нужно отдышаться!",
        3.7
    ),
    entry!(
        29,
        "error-state-pixel",
        "Розовое предупреждение",
        Error,
        "Ой, что-то пошло не так!",
        3.8
    ),
    entry!(
        30,
        "examining-green-globe",
        "Зелёный шар",
        Reflection,
        "Рассмотрю шар поближе!",
        3.7
    ),
    entry!(
        31,
        "failed-process-error",
        "Неудачная попытка",
        Error,
        "Попробую ещё раз!",
        3.8
    ),
    entry!(
        32,
        "fast-performance-trail",
        "Скоростной след",
        Strength,
        "Быстро-быстро!",
        3.5
    ),
    entry!(
        33,
        "fine-under-pressure",
        "Спокойствие в огне",
        Mood,
        "Дышу ровно и спокойно.",
        4.0
    ),
    entry!(
        34,
        "growing-pixel-nature",
        "Пиксельная зелень",
        Nature,
        "Пусть всё растёт!",
        4.1
    ),
    entry!(
        35,
        "happy-pixel-celebrating",
        "Радостный танец",
        Celebration,
        "Ура, получилось!",
        4.0
    ),
    entry!(
        36,
        "holding-ancient-scroll",
        "Древний свиток",
        Reflection,
        "Читаю старый свиток!",
        3.9
    ),
    entry!(
        37,
        "holding-green-leaf",
        "Зелёный лист",
        Nature,
        "Бережно держу листочек.",
        3.7
    ),
    entry!(
        38,
        "idea-lightbulb-pixel",
        "Лампочка идеи",
        Reflection,
        "Кажется, придумал!",
        4.0
    ),
    entry!(
        39,
        "in-constant-loop",
        "Кольцо пикселей",
        Coding,
        "Пиксель за пикселем!",
        3.8
    ),
    entry!(
        40,
        "in-glowing-aura",
        "Мягкое сияние",
        Mood,
        "Сияю понемногу!",
        3.7
    ),
    entry!(
        41,
        "infrastructure-server-stacks",
        "Цветные серверы",
        Security,
        "Соберу башню из кубиков!",
        4.0
    ),
    entry!(
        42,
        "it-depends-sign",
        "Знак зависимости",
        Reflection,
        "Тут всё зависит от контекста!",
        3.8
    ),
    entry!(
        43,
        "lab-science-testing",
        "Пиксельная лаборатория",
        Building,
        "Ставлю опыт на стол!",
        3.9
    ),
    entry!(
        44,
        "lifting-heavy-barbell",
        "Пиксельная штанга",
        Strength,
        "Подниму на счёт три!",
        4.1
    ),
    entry!(
        45,
        "love-cloud-pixel",
        "Облачко любви",
        Celebration,
        "Посылаю тёплый привет!",
        4.0
    ),
    entry!(
        46,
        "malware-detection-window",
        "Окошко с пикселями",
        Security,
        "Разглядываю пиксельное окно.",
        3.9
    ),
    entry!(
        47,
        "managing-server-rack",
        "Стойка из кубиков",
        Security,
        "Расставлю кубики ровно!",
        4.0
    ),
    entry!(
        48,
        "marking-critical-error",
        "Заметка на экране",
        Error,
        "Отмечу эту деталь!",
        3.8
    ),
    entry!(
        49,
        "multi-window-workflow",
        "Много окошек",
        Coding,
        "Переключаю окошки!",
        3.9
    ),
    entry!(
        50,
        "nature-growth-mushrooms",
        "Пиксельные грибочки",
        Nature,
        "Грибочки подрастают!",
        4.1
    ),
    entry!(
        51,
        "planting-small-trees",
        "Маленькие деревца",
        Nature,
        "Посажу ещё росток!",
        4.0
    ),
    entry!(
        52,
        "processing-logic-symbols",
        "Знаки логики",
        Coding,
        "Сложу знаки в ряд!",
        3.9
    ),
    entry!(
        53,
        "professional-documentation-audit",
        "Аккуратные заметки",
        Coding,
        "Проверю записи в рисунке.",
        3.8
    ),
    entry!(
        54,
        "reading-data-document",
        "Чтение документа",
        Coding,
        "Читаю пиксельную страницу!",
        3.8
    ),
    entry!(
        55,
        "refresh-sync-pixel",
        "Круг обновления",
        Coding,
        "Кручу пиксельный цикл!",
        3.7
    ),
    entry!(
        56,
        "repairing-with-tools",
        "Инструменты ремонта",
        Building,
        "Починю игрушечную деталь!",
        4.0
    ),
    entry!(
        57,
        "reviewing-long-data-log",
        "Длинная лента",
        Coding,
        "Листаю длинную ленту!",
        3.9
    ),
    entry!(
        58,
        "rocket-launch-success",
        "Пиксельная ракета",
        Celebration,
        "Три, два, один, полёт!",
        4.1
    ),
    entry!(
        59,
        "secure-access-padlock",
        "Замочек с ключом",
        Security,
        "Закрою замочек!",
        3.9
    ),
    entry!(
        60,
        "server-stack-pixel",
        "Стопка серверов",
        Security,
        "Считаю стопку кубиков!",
        3.9
    ),
    entry!(
        61,
        "sleeping-soundly",
        "Крепкий сон",
        Mood,
        "Тихо сплю...",
        4.2
    ),
    entry!(
        62,
        "stealth-ninja-security",
        "Тихий ниндзя",
        Security,
        "Тихо ступаю по пикселям!",
        3.9
    ),
    entry!(
        63,
        "success-achievement-milestone",
        "Новый рубеж",
        Celebration,
        "Вот это достижение!",
        4.0
    ),
    entry!(
        64,
        "system-error-alert",
        "Розовая карточка",
        Error,
        "Смотрю на карточку.",
        3.8
    ),
    entry!(
        65,
        "system-monitoring-dashboard",
        "Пиксельная панель",
        Coding,
        "Смотрю на огоньки панели!",
        3.9
    ),
    entry!(
        66,
        "thinking-in-code",
        "Мысли в коде",
        Reflection,
        "Думаю над строкой кода.",
        3.9
    ),
    entry!(
        67,
        "thinking-pixel-bubble",
        "Пузырёк мыслей",
        Reflection,
        "Мысль всплыла!",
        3.8
    ),
    entry!(
        68,
        "tracking-multiple-locations",
        "Точки на карте",
        Reflection,
        "Поставлю точки на карту!",
        3.9
    ),
    entry!(
        69,
        "trending-up-pixel",
        "Растущая стрелка",
        Celebration,
        "Стрелка ползёт вверх!",
        3.9
    ),
    entry!(
        70,
        "user-profile-settings",
        "Настройки аватара",
        Coding,
        "Подберу цвета аватару!",
        3.9
    ),
    entry!(
        71,
        "visualizing-tree-data",
        "Дерево данных",
        Coding,
        "Рисую ветви данных!",
        3.9
    ),
    entry!(
        72,
        "welcome-pixel-banner",
        "Добро пожаловать",
        Celebration,
        "Рад тебя видеть!",
        4.0
    ),
    entry!(
        73,
        "with-success-checkmark",
        "Зелёная галочка",
        Celebration,
        "Готово, ставлю галочку!",
        3.9
    ),
    entry!(
        74,
        "writing-software-code",
        "Пиксельный код",
        Coding,
        "Печатаю пиксельный код!",
        3.9
    ),
];

/// Draw props and cosmetic effects for one activity. The caller draws its
/// canonical Claude body separately; this function never paints a mascot.
pub fn draw_activity(canvas: &mut Canvas, id: ActivityId, elapsed: f64, facing: f64, gaze: Vec2) {
    let id = if id.index() < SCENE_COUNT {
        id
    } else {
        ActivityId(0)
    };
    let (min_x, min_y, max_x, max_y, body_min_x, _body_min_y, body_max_x, body_max_y, cells) =
        scene(id.index());
    let body_cx = (body_min_x + body_max_x) as f64 * 0.5;
    let feet_y = FEET_Y.min(canvas.height as f64 - 1.0);
    // A source grid cell maps to five renderer pixels, matching Claude's
    // canonical pixel unit. Large scenes may only scale down to fit bounds.
    let mut scale = 5.0_f64;
    if max_x > min_x && max_y > min_y {
        let scene_w = (max_x - min_x) as f64 * GRID_UNIT;
        let scene_h = (max_y - min_y) as f64 * GRID_UNIT;
        scale = scale.min(140.0 / scene_w).min(128.0 / scene_h);
        scale = scale.min(FEET_X / ((body_cx - min_x as f64).max(0.5) * GRID_UNIT));
        scale = scale
            .min((canvas.width as f64 - FEET_X) / ((max_x as f64 - body_cx).max(0.5) * GRID_UNIT));
        scale = scale.min(feet_y / ((body_max_y - min_y).max(1) as f64 * GRID_UNIT));
        if max_y > body_max_y {
            scale = scale.min(
                (canvas.height as f64 - feet_y - 1.0) / ((max_y - body_max_y) as f64 * GRID_UNIT),
            );
        }
    }
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let cell_size = GRID_UNIT * scale;
    // Effects use the renderer-owned base mascot's stable bounds, never an
    // imported activity's body dimensions.
    let width = 50.0;
    let height = 62.0;
    let left = FEET_X - width * 0.5;
    let top = feet_y - height;

    let t = elapsed.max(0.0);
    let duration = id.duration();
    let reveal = (t / 0.26).clamp(0.0, 1.0);
    let close = ((duration - t) / 0.34).clamp(0.0, 1.0);
    let alpha = (reveal.min(close) * 255.0).round() as u32;
    let (chunks, remainder) = cells.as_chunks::<8>();
    debug_assert!(remainder.is_empty());
    let suppress_source_props = matches!(
        id.spec().slug,
        "angry-powerful-lightning"
            | "developer-pair-programming-scene"
            | "artboard-2cla3"
            | "malware-detection-window"
            | "reading-data-document"
            | "stealth-ninja-security"
    );
    let floor_lift = (max_y.saturating_sub(body_max_y) as f64 * cell_size).max(0.0);
    for chunk in chunks.iter().filter(|_| !suppress_source_props) {
        let x = chunk[0] as f64;
        let y = chunk[1] as f64;
        let w = chunk[2] as f64;
        let h = chunk[3] as f64;
        let rgb = ((chunk[4] as u32) << 16) | ((chunk[5] as u32) << 8) | chunk[6] as u32;
        let color = (alpha << 24) | rgb;
        let tag = chunk[7];
        let mirror = facing < 0.0;
        let source_x = if mirror { 2.0 * body_cx - x - w } else { x };
        let px = FEET_X + (source_x - body_cx) * cell_size;
        let mut py = feet_y - (body_max_y as f64 - y) * cell_size - floor_lift;
        if tag == 1 {
            let dy = match id.spec().slug {
                "rocket-launch-success" => -((t - 0.55).max(0.0) * 7.0).min(12.0),
                "lifting-heavy-barbell" => {
                    let beat = (t * 1.9).fract();
                    if beat > 0.19 && beat < 0.59 {
                        -4.5
                    } else {
                        0.0
                    }
                }
                "baking-steaming-pie" => (t * 4.0).sin() * 1.5,
                "planting-small-trees" | "nature-growth-mushrooms" | "growing-pixel-nature" => {
                    (t * 2.2).sin() * 1.0
                }
                _ => 0.0,
            };
            py += dy;
        }
        canvas.rect(px, py, w * cell_size, h * cell_size, color);
    }
    if alpha < 24 {
        return;
    }
    draw_effect(
        canvas,
        EffectContext {
            id,
            group: id.spec().group,
            t,
            alpha,
            facing,
            gaze,
            left,
            top,
            width,
            height,
        },
    );
}

fn scene(index: usize) -> (u8, u8, u8, u8, u8, u8, u8, u8, &'static [u8]) {
    let bytes = SCENE_DATA;
    if bytes.len() < 5 + SCENE_COUNT * 4
        || &bytes[..4] != b"APSP"
        || bytes[4] as usize != SCENE_COUNT
    {
        return (0, 0, 0, 0, 0, 0, 0, 0, &[]);
    }
    let index_at = 5 + index * 4;
    let offset = u32::from_le_bytes(bytes[index_at..index_at + 4].try_into().unwrap()) as usize;
    if offset + 10 > bytes.len() {
        return (0, 0, 0, 0, 0, 0, 0, 0, &[]);
    }
    let min_x = bytes[offset];
    let min_y = bytes[offset + 1];
    let max_x = bytes[offset + 2];
    let max_y = bytes[offset + 3];
    let body_min_x = bytes[offset + 4];
    let body_min_y = bytes[offset + 5];
    let body_max_x = bytes[offset + 6];
    let body_max_y = bytes[offset + 7];
    let count = u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]) as usize;
    let start = offset + 10;
    let end = match start.checked_add(count.saturating_mul(8)) {
        Some(end) if end <= bytes.len() => end,
        _ => return (0, 0, 0, 0, 0, 0, 0, 0, &[]),
    };
    (
        min_x,
        min_y,
        max_x,
        max_y,
        body_min_x,
        body_min_y,
        body_max_x,
        body_max_y,
        &bytes[start..end],
    )
}

struct EffectContext {
    id: ActivityId,
    group: ActivityGroup,
    t: f64,
    alpha: u32,
    facing: f64,
    gaze: Vec2,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

fn draw_effect(canvas: &mut Canvas, effect: EffectContext) {
    let EffectContext {
        id,
        group,
        t,
        alpha,
        facing,
        gaze,
        left,
        top,
        width,
        height,
    } = effect;
    let pulse = 0.5 + 0.5 * (t * 7.5).sin();
    let a = (alpha as f64 * (0.45 + pulse * 0.45)).round() as u32;
    let dir = if facing < 0.0 { -1.0 } else { 1.0 };
    let gx = gaze.x.clamp(-1.0, 1.0) * 2.0;
    let gy = gaze.y.clamp(-1.0, 1.0) * 1.0;
    let cx = left + width * 0.5 + gx;
    let prop = left + width * (if dir > 0.0 { 0.73 } else { 0.27 });
    let sky = (top - 6.0).max(42.0);
    let floor = FEET_Y - 15.0;
    match group {
        ActivityGroup::Food => {
            // Three separate soft puffs loop upward above the food prop.
            for n in 0..3 {
                let phase = (t * 1.1 + n as f64 * 0.34).fract();
                let x = prop + ((n as f64 - 1.0) * 5.0);
                let y = floor - 34.0 - phase * 23.0;
                canvas.rect(x.round(), (y + gy).round(), 3.0, 3.0, (a << 24) | 0xD8E4E7);
                canvas.rect(
                    (x + 3.0).round(),
                    (y - 3.0 + gy).round(),
                    2.0,
                    2.0,
                    (a << 24) | 0xF7F1DE,
                );
            }
        }
        ActivityGroup::Nature => {
            let grow = (t * 0.8).fract();
            canvas.rect(
                (cx - 1.0).round(),
                (floor - grow * 9.0).round(),
                2.0,
                5.0 + grow * 5.0,
                (a << 24) | 0x27B262,
            );
            canvas.rect(
                (cx - 5.0).round(),
                (floor - 7.0 - grow * 5.0).round(),
                4.0,
                2.0,
                (a << 24) | 0x5BEF7E,
            );
            canvas.rect(
                (cx + 1.0).round(),
                (floor - 11.0 - grow * 5.0).round(),
                4.0,
                2.0,
                (a << 24) | 0x169986,
            );
        }
        ActivityGroup::Coding | ActivityGroup::Building => {
            // A blinking pixel cursor and a small stroke of code/assembly.
            let on = (t * 3.0).fract() < 0.64;
            let px = prop + dir * 8.0;
            if on {
                canvas.rect(
                    px.round(),
                    (floor - 32.0 + gy).round(),
                    3.0,
                    5.0,
                    (a << 24) | 0x51F4A3,
                );
            }
            canvas.rect(
                (px - 5.0).round(),
                (floor - 23.0 + gy).round(),
                8.0,
                2.0,
                (a << 24) | 0x55CFF4,
            );
            canvas.rect(
                (px - 2.0).round(),
                (floor - 19.0 + gy).round(),
                6.0,
                2.0,
                (a << 24) | 0xFFCC52,
            );
            if group == ActivityGroup::Building {
                let beat = (t * 1.4).fract();
                canvas.rect(
                    (cx - 1.5).round(),
                    (floor - 5.0 - beat * 7.0).round(),
                    5.0,
                    5.0,
                    (a << 24) | 0xDA64DD,
                );
            }
        }
        ActivityGroup::Celebration => {
            // Short irregular burst; particles rise from the lifted-hand area,
            // then fall/fade instead of making the whole character bob.
            let cycle = (t * 0.9).fract();
            for n in 0..7 {
                let seed = n as f64 * 1.618 + 0.37;
                let x = cx + (seed.sin() * 25.0);
                let y = sky + ((seed * 4.7).fract() * 12.0) + cycle * 18.0;
                let fade = alpha as f64 * (1.0 - cycle * 0.75);
                let colors = [0xFFFFC431, 0xFF55E233, 0xFFEA3F99, 0xFF55A8FF];
                let c = colors[n % colors.len()];
                canvas.rect(
                    x.round(),
                    y.round(),
                    if n % 2 == 0 { 3.0 } else { 2.0 },
                    3.0,
                    ((fade as u32) << 24) | (c & 0x00FFFFFF),
                );
            }
        }
        ActivityGroup::Error => {
            // Pulse a small outlined warning tile, with uneven pixel cadence.
            let x = prop;
            let y = sky + 4.0;
            let c = (a << 24) | 0xFF738E;
            canvas.rect(x.round(), y.round(), 8.0, 2.0, c);
            canvas.rect(x.round(), (y + 2.0).round(), 2.0, 5.0, c);
            canvas.rect((x + 6.0).round(), (y + 2.0).round(), 2.0, 5.0, c);
            canvas.rect(
                (x + 3.0).round(),
                (y + if pulse > 0.5 { 2.0 } else { 4.0 }).round(),
                2.0,
                3.0,
                c,
            );
            if pulse > 0.78 {
                canvas.rect((x + 3.0).round(), (y + 7.0).round(), 2.0, 2.0, c);
            }
        }
        ActivityGroup::Security => {
            // Status lamps blink in a staggered sequence as a harmless prop.
            for n in 0..3 {
                let lit = (t * 2.2 + n as f64 * 0.31).fract() < 0.48;
                let color = if lit {
                    [0x66E879, 0x4FC8FF, 0xFFC431][n]
                } else {
                    0x34545C
                };
                canvas.rect(
                    (prop + n as f64 * 4.0 - 4.0).round(),
                    (floor - 20.0).round(),
                    3.0,
                    3.0,
                    (a << 24) | color,
                );
            }
        }
        ActivityGroup::Mood => {
            // Light motion accents; crying scenes get explicitly animated drops.
            let tears = matches!(id.spec().slug, "crying-sad-reaction" | "broken-heart-sad");
            if tears {
                for n in 0..2 {
                    let phase = (t * 1.3 + n as f64 * 0.5).fract();
                    canvas.rect(
                        (cx + (n as f64 * 5.0 - 4.0)).round(),
                        (top + height * 0.42 + phase * 12.0).round(),
                        2.0,
                        3.0,
                        (a << 24) | 0x3EADFF,
                    );
                }
            } else if pulse > 0.62 {
                canvas.rect(
                    (cx + dir * 23.0).round(),
                    (top + height * 0.28 + gy).round(),
                    2.0,
                    2.0,
                    (a << 24) | 0xFFF2A4,
                );
            }
        }
        ActivityGroup::Reflection => {
            // Orbiting thought pixels sit near the upper prop without shifting feet.
            let angle = t * 2.2;
            let x = cx + angle.cos() * 10.0;
            let y = sky + 15.0 + angle.sin() * 5.0;
            canvas.rect(x.round(), y.round(), 3.0, 3.0, (a << 24) | 0xA88AF4);
            canvas.rect(
                (x - 5.0).round(),
                (y + 4.0).round(),
                2.0,
                2.0,
                (a << 24) | 0x55D8B2,
            );
        }
        ActivityGroup::Strength => {
            // A small effort sparkle; prop movement is applied to the imported
            // barbell component and no body/hand/limb pixels are added here.
            let beat = (t * 1.9).fract();
            let raised = beat > 0.19 && beat < 0.55;
            canvas.rect(
                (cx - 1.0).round(),
                (floor - if raised { 39.0 } else { 35.0 }).round(),
                2.0,
                2.0,
                (a << 24) | 0xFFC431,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_75_unique_stable_entries_and_bounded_durations() {
        assert_eq!(CATALOG.len(), 75);
        for (index, spec) in CATALOG.iter().enumerate() {
            assert_eq!(spec.id.index(), index);
            assert!((3.0..=5.0).contains(&spec.duration));
            assert!(!spec.slug.is_empty() && !spec.title_ru.is_empty());
            assert!(CATALOG[..index].iter().all(|other| other.slug != spec.slug));
        }
        assert!(ActivityId::new(74).is_some());
        assert!(ActivityId::new(75).is_none());
    }

    #[test]
    fn compiled_pack_has_all_well_formed_props_only_scenes() {
        assert_eq!(&SCENE_DATA[..4], b"APSP");
        assert_eq!(SCENE_DATA[4] as usize, SCENE_COUNT);
        for i in 0..SCENE_COUNT {
            let (x0, y0, x1, y1, bx0, by0, bx1, by1, cells) = scene(i);
            assert!(bx1 > bx0 && by1 > by0);
            assert!(x1 <= 64 && y1 <= 64);
            assert!(cells.len() % 8 == 0);
            let (chunks, remainder) = cells.as_chunks::<8>();
            assert!(remainder.is_empty());
            for c in chunks {
                assert!(c[2] > 0 && c[3] > 0);
                assert!(
                    c[2] <= 1 && c[3] <= 1,
                    "scene {i} cell exceeds the 5px source grid"
                );
                assert!(c[7] <= 1);
                assert!(c[0] >= x0 && c[1] >= y0);
                assert!(c[0] + c[2] <= x1 && c[1] + c[3] <= y1);
                let rgb = (u32::from(c[4]) << 16) | (u32::from(c[5]) << 8) | u32::from(c[6]);
                assert_ne!(rgb, 0xD97757, "scene {i} includes imported mascot orange");
                let in_face = c[0] >= bx0 && c[0] < bx1 && c[1] >= by0 && c[1] < by1;
                if in_face {
                    let min = c[4].min(c[5]).min(c[6]);
                    let max = c[4].max(c[5]).max(c[6]);
                    let dark_or_white_face =
                        max <= 90 || min >= 232 || (max - min <= 12 && (max <= 150 || min >= 230));
                    assert!(!dark_or_white_face, "scene {i} includes source face pixels");
                }
            }
        }
    }

    #[test]
    fn activity_layer_never_draws_the_imported_mascot_body_or_face() {
        for i in 0..SCENE_COUNT {
            let mut canvas = Canvas::new(280, 240);
            draw_activity(&mut canvas, ActivityId(i as u8), 1.2, 1.0, Vec2::default());
            assert!(
                !canvas.pixels.contains(&0xFFD97757),
                "scene {i} drew source body orange"
            );
            assert!(
                canvas.pixels.iter().any(|&p| p != 0),
                "scene {i} has no prop/effect activity"
            );
        }
    }

    #[test]
    fn animated_accents_change_without_moving_the_floor_anchor() {
        let id = ActivityId::new(19).unwrap(); // pie steam
        let mut first = Canvas::new(280, 240);
        let mut second = Canvas::new(280, 240);
        draw_activity(&mut first, id, 1.0, 1.0, Vec2::default());
        draw_activity(&mut second, id, 1.5, 1.0, Vec2::default());
        assert_ne!(first.pixels, second.pixels);
        for i in [0, 19, 34, 44, 57, 66, 72] {
            let mut canvas = Canvas::new(280, 240);
            draw_activity(&mut canvas, ActivityId(i), 1.2, 1.0, Vec2::new(0.4, -0.2));
            assert_eq!(canvas.pixels.len(), 280 * 240);
            assert!(canvas.pixels.iter().any(|&p| p != 0));
        }
    }

    #[test]
    fn every_catalog_scene_changes_during_its_active_timeline() {
        let mut first = Canvas::new(280, 240);
        let mut second = Canvas::new(280, 240);
        for spec in &CATALOG {
            first.clear();
            second.clear();
            draw_activity(&mut first, spec.id, 1.13, 1.0, Vec2::default());
            draw_activity(&mut second, spec.id, 1.61, 1.0, Vec2::default());
            assert_ne!(
                first.pixels, second.pixels,
                "{} must animate during its active phase",
                spec.slug
            );
            first.clear();
            draw_activity(
                &mut first,
                spec.id,
                spec.duration + 0.1,
                1.0,
                Vec2::default(),
            );
            assert!(
                first.pixels.iter().all(|&pixel| pixel == 0),
                "{} must remove all props after closing",
                spec.slug
            );
        }
    }
}
