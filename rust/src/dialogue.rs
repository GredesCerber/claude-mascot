//! Local, deterministic speech selection independent from the physics RNG.
use crate::activities::ActivityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum SpeechEvent {
    Wake,
    Wander,
    Laptop,
    Stretch,
    LookAround,
    RoughThrow,
    Petted,
    Falling,
    FolderOpened,
    NewWindow,
    VideoPeek,
    CursorHold,
    Dance,
    Wave,
    Chase,
    Jump,
    Greeting,
    GreetingReply,
    HighFive,
    HighFiveReply,
    DanceTogether,
    ShareLaptop,
    WatchLaptop,
    Comfort,
    ComfortReply,
    TagChaser,
    TagRunner,
    TowerTop,
    LeapfrogJump,
    LeapfrogWait,
    SocialInvite,
    TowerLowCeiling,
    TowerNoRoom,
    FolderFocused,
    Climb,
    Rest,
    Sleep,
}
pub const EVENT_COUNT: usize = SpeechEvent::Sleep as usize + 1;
pub const ALL_EVENTS: [SpeechEvent; EVENT_COUNT] = [
    SpeechEvent::Wake,
    SpeechEvent::Wander,
    SpeechEvent::Laptop,
    SpeechEvent::Stretch,
    SpeechEvent::LookAround,
    SpeechEvent::RoughThrow,
    SpeechEvent::Petted,
    SpeechEvent::Falling,
    SpeechEvent::FolderOpened,
    SpeechEvent::NewWindow,
    SpeechEvent::VideoPeek,
    SpeechEvent::CursorHold,
    SpeechEvent::Dance,
    SpeechEvent::Wave,
    SpeechEvent::Chase,
    SpeechEvent::Jump,
    SpeechEvent::Greeting,
    SpeechEvent::GreetingReply,
    SpeechEvent::HighFive,
    SpeechEvent::HighFiveReply,
    SpeechEvent::DanceTogether,
    SpeechEvent::ShareLaptop,
    SpeechEvent::WatchLaptop,
    SpeechEvent::Comfort,
    SpeechEvent::ComfortReply,
    SpeechEvent::TagChaser,
    SpeechEvent::TagRunner,
    SpeechEvent::TowerTop,
    SpeechEvent::LeapfrogJump,
    SpeechEvent::LeapfrogWait,
    SpeechEvent::SocialInvite,
    SpeechEvent::TowerLowCeiling,
    SpeechEvent::TowerNoRoom,
    SpeechEvent::FolderFocused,
    SpeechEvent::Climb,
    SpeechEvent::Rest,
    SpeechEvent::Sleep,
];
impl SpeechEvent {
    pub fn lines(self) -> &'static [&'static str] {
        match self {
            Self::Wake => &[
                "Проснулся!",
                "Я тут!",
                "Так, что пропустил?",
                "Снова в деле!",
                "О, ты вернулся!",
                "Крабик на связи!",
                "Ну что, продолжаем?",
                "Выспался :)",
            ],
            Self::Wander => &[
                "Пойду прогуляюсь",
                "Разомну лапки",
                "Я тут побегаю",
                "Проверю, что за краем",
                "Далеко не уйду",
            ],
            Self::Laptop => &[
                "Сейчас допечатаю",
                "Где моя клавиатура?",
                "Лапки на клавиши!",
                "Есть одна идейка",
                "Так, строчку сюда...",
                "Сосредоточенный краб",
            ],
            Self::Stretch => &[
                "Разомнусь немного",
                "Лапки затекли",
                "Во-о-от так!",
                "Потяну-у-улись",
                "Зарядка для крабика",
            ],
            Self::LookAround => &[
                "Что тут интересного?",
                "Огляжусь немного",
                "Так-так...",
                "Что за движ?",
                "Я всё вижу!",
            ],
            Self::RoughThrow => &[
                "Эй, аккуратнее!",
                "Я же не мячик!",
                "Полегче с крабиком",
                "Вот это встряска!",
                "Лапки побереги!",
            ],
            Self::Petted => &[
                "Приятно :) ",
                "Мр-р... краб-р-р",
                "Ещё немного?",
                "Вот так хорошо",
                "Лапки довольны!",
                "Я уже добрее",
            ],
            Self::Falling => &[
                "А-а-а!",
                "Лечу-у!",
                "Посадку разрешаете?",
                "Где тормоза?!",
                "Держите крабика!",
            ],
            Self::FolderOpened => &[
                "О, папка!",
                "Папка открылась!",
                "Ого, что там?",
                "Чего ищем?",
                "И мне покажи!",
                "Так, что открываем?",
                "Ищем что-то важное?",
                "Папки-папочки!",
                "Любопытно...",
            ],
            Self::NewWindow => &[
                "О, новое окошко!",
                "Кто тут у нас?",
                "Что открыли?",
                "Ого!",
                "Интересно, что там?",
            ],
            Self::VideoPeek => &[
                "Что смотрим?",
                "И я посмотрю",
                "Кино для крабика?",
                "Можно с краешку?",
                "Устроюсь поудобнее",
            ],
            Self::CursorHold => &[
                "Вишу!",
                "Поймал курсор!",
                "Держусь лапками!",
                "Подвези меня!",
                "Я прицепился!",
            ],
            Self::Dance => &[
                "Лапки сами пляшут",
                "Музыку в студию!",
                "Крабий танец!",
                "Раз-два, раз-два!",
                "А вот и мой номер",
            ],
            Self::Wave => &[
                "Привет-привет!",
                "Я здесь!",
                "Лапкой машу!",
                "Вижу тебя!",
                "Крабик передаёт привет",
            ],
            Self::Chase => &[
                "А ну, постой!",
                "Догоню курсор!",
                "Лапки, вперёд!",
                "Куда это ты?",
                "За этой стрелочкой!",
            ],
            Self::Jump => &[
                "Прыг!",
                "Выше!",
                "Лапки от пола!",
                "Смотри, как умею!",
                "Сейчас взлечу!",
            ],
            Self::Greeting => &[
                "Привет!",
                "О, дружище!",
                "Как лапки?",
                "Рад тебя видеть!",
                "Крабик, привет!",
            ],
            Self::GreetingReply => &[
                "Привет :) ",
                "И тебе привет!",
                "Я тут!",
                "Всё хорошо!",
                "Рад компании!",
            ],
            Self::HighFive => &["Дай пять!", "Лапку!", "Крабьи пять!", "Пять сюда!"],
            Self::HighFiveReply => &["Пять!", "Держи!", "Лови лапку!", "Есть контакт!"],
            Self::DanceTogether => &[
                "Потанцуем?",
                "Лапки в такт!",
                "Наш выход!",
                "Раз-два!",
                "Вот это ритм!",
            ],
            Self::ShareLaptop => &[
                "Смотри сюда",
                "Есть идея!",
                "Глянь на код",
                "Вот эту строчку?",
                "Разберёмся вместе",
            ],
            Self::WatchLaptop => &[
                "Вижу!",
                "Ага, понял",
                "Интересно!",
                "Покажи ещё",
                "Давай подумаем",
            ],
            Self::Comfort => &[
                "Я рядом",
                "Всё наладится",
                "Держись, друг",
                "Лапку помощи?",
                "Давай передохнём",
            ],
            Self::ComfortReply => &[
                "Спасибо!",
                "Уже легче",
                "Хорошо, что ты тут",
                "Выдохнул...",
                "Дружеская лапка :)",
            ],
            Self::TagChaser => &[
                "Догоню!",
                "Ты водишь?",
                "Попался!",
                "Лови крабика!",
                "Сейчас поймаю!",
            ],
            Self::TagRunner => &[
                "Не поймаешь!",
                "А я быстрее!",
                "Убегаю!",
                "Лапки, не подведите!",
                "Догони сначала!",
            ],
            Self::TowerTop => &[
                "Я сверху!",
                "Выше всех!",
                "Держусь!",
                "Вот это вид!",
                "Вершина!",
            ],
            Self::LeapfrogJump => &["Прыгаю!", "Прыг-скок!", "Мой ход!", "Лечу!"],
            Self::LeapfrogWait => &["Прыгай!", "Жду!", "Давай!", "Твой ход!"],
            Self::SocialInvite => &[
                "Кто со мной?",
                "Поиграем?",
                "Есть желающие?",
                "Друзья, я тут!",
                "Крабья компания!",
            ],
            Self::TowerLowCeiling => &[
                "Потолок слишком низкий",
                "Тут башня не поместится",
                "Нужен потолок повыше",
                "Головой в потолок упрусь",
            ],
            Self::TowerNoRoom => &[
                "Мало места для башни",
                "Нужна площадка пошире",
                "Тут тесновато для всех",
                "Для башни нужна опора шире",
            ],
            Self::FolderFocused => &[
                "Снова к папкам?",
                "Чего ищем?",
                "Я тоже посмотрю",
                "Что у нас здесь?",
                "Нашлось что-нибудь?",
                "Папочные дела!",
            ],
            Self::Climb => &[
                "Попробую забраться",
                "Лапками вверх!",
                "Что там наверху?",
                "Найду за что зацепиться",
            ],
            Self::Rest => &[
                "Посижу немножко",
                "Передышка для лапок",
                "Мне и тут хорошо",
                "Устроюсь поудобнее",
            ],
            Self::Sleep => &[
                "Разбудишь потом?",
                "Ушёл в крабьи сны",
                "Лапки отдыхают",
                "Ещё пять минуточек...",
            ],
        }
    }
    fn ambient(self) -> bool {
        matches!(self, Self::NewWindow | Self::VideoPeek)
    }
}

const KEYS: usize = EVENT_COUNT + 75;
#[derive(Clone, Debug)]
pub struct Dialogue {
    rng: u64,
    previous: [u8; KEYS],
    recent: [Option<&'static str>; 4],
    recent_at: usize,
    last_key: Option<usize>,
    last_time: f64,
    ambient_after: f64,
}
impl Dialogue {
    pub fn new(pet_id: u64) -> Self {
        Self {
            rng: pet_id
                .wrapping_mul(0xD1342543DE82EF95)
                .wrapping_add(0x9E3779B97F4A7C15),
            previous: [u8::MAX; KEYS],
            recent: [None; 4],
            recent_at: 0,
            last_key: None,
            last_time: f64::NEG_INFINITY,
            ambient_after: 0.0,
        }
    }
    pub fn event(&mut self, event: SpeechEvent, now: f64) -> Option<&'static str> {
        self.select(event as usize, event.lines(), now, event.ambient())
    }
    pub fn activity(&mut self, id: ActivityId, now: f64) -> Option<&'static str> {
        let bank = crate::activity_dialogue::ACTIVITY_LINES.get(id.index())?;
        self.select(EVENT_COUNT + id.index(), bank, now, false)
    }
    fn select(
        &mut self,
        key: usize,
        bank: &'static [&'static str],
        now: f64,
        ambient: bool,
    ) -> Option<&'static str> {
        if !now.is_finite()
            || bank.is_empty()
            || (self.last_key == Some(key) && now - self.last_time < 1.2)
            || (ambient && now < self.ambient_after)
        {
            return None;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let start = (self.rng as usize) % bank.len();
        let index = (0..bank.len())
            .map(|offset| (start + offset) % bank.len())
            .find(|&i| self.previous[key] != i as u8 && !self.recent.contains(&Some(bank[i])))
            .or_else(|| {
                (0..bank.len())
                    .map(|offset| (start + offset) % bank.len())
                    .find(|&i| self.previous[key] != i as u8)
            })
            .unwrap_or(start);
        let text = bank[index];
        self.previous[key] = index as u8;
        self.recent[self.recent_at] = Some(text);
        self.recent_at = (self.recent_at + 1) % 4;
        self.last_key = Some(key);
        self.last_time = now;
        self.ambient_after = now + 4.0;
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_have_short_distinct_variants_and_no_instant_repeats() {
        for event in ALL_EVENTS {
            let bank = event.lines();
            assert!(bank.len() >= 4);
            let unique: std::collections::HashSet<_> = bank.iter().collect();
            assert_eq!(unique.len(), bank.len(), "{event:?}");
            assert!(bank
                .iter()
                .all(|line| !line.trim().is_empty() && line.chars().count() <= 40));
        }
        let mut speech = Dialogue::new(7);
        let mut previous = None;
        let mut seen = std::collections::HashSet::new();
        for step in 0..40 {
            let line = speech
                .event(SpeechEvent::FolderOpened, step as f64 * 2.0)
                .unwrap();
            assert_ne!(Some(line), previous);
            assert!(line.chars().count() <= 40);
            seen.insert(line);
            previous = Some(line);
        }
        assert!(seen.len() >= 4);
        assert!(speech.event(SpeechEvent::FolderOpened, 78.1).is_none());
    }
    #[test]
    fn every_activity_uses_its_bank_and_keeps_recent_choices_varied() {
        for spec in crate::activities::CATALOG {
            let bank = crate::activity_dialogue::ACTIVITY_LINES[spec.id.index()];
            assert!(bank.len() >= 4, "{}", spec.slug);
            assert!(bank
                .iter()
                .all(|s| !s.trim().is_empty() && s.chars().count() <= 40));
            let mut speech = Dialogue::new(8);
            let mut last = None;
            for step in 0..12 {
                let line = speech.activity(spec.id, step as f64 * 2.0).unwrap();
                assert!(bank.contains(&line));
                assert_ne!(Some(line), last);
                last = Some(line);
            }
        }
    }
    #[test]
    fn ambient_chatter_does_not_replace_a_fresh_action() {
        let mut speech = Dialogue::new(1);
        assert!(speech.event(SpeechEvent::Wake, 0.0).is_some());
        assert!(speech.event(SpeechEvent::NewWindow, 1.0).is_none());
        assert!(speech.event(SpeechEvent::FolderOpened, 1.0).is_some());
    }
}
