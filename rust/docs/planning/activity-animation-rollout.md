# Полноценные действия для всех 75 иллюстраций

Источники: [пак](https://getillustrations.com/illustration-pack/claude-mascot-pack), [Codrops](https://tympanus.net/codrops/2026/05/05/reverse-engineering-claude-ais-mascot-animations-with-svg-and-gsap/).

## Порядок и критерии

1. Простые жесты и один предмет (23 сцены).
2. Панели, природные и эмоциональные сцены.
3. Составные действия: ноутбук, ремонт, строительство, ракета и штанга.

Каждая сцена получает начало, основное действие и завершение; общий эффект категории не считается законченной анимацией. Предмет и ладонь используют общую геометрию. Верх тела может смещаться, лапки опираются на пол. Тело и лицо рисует единственный renderer, используется выбранная пользователем палитра. Позы 12 Гц, физика отдельно. Новые люди и дополнительные нарисованные персонажи запрещены. Старые ID0–74 не меняются.

Проверки: обе стороны, 75–300%, передний/задний слой, отсутствие предметов после конца, ручная отмена, отсутствие пересечения глаз, общий каталог/меню/реплики. После кода — просмотр экспортированных кадров с отдельным ревью Terra.

| Этап | ID | Сцена | Исполнитель | Приёмка |
| --- | --- | --- | --- | --- |
| 1 | 1 | Крепкий кофе (`angry-pixel-coffee`) | Sol simple | Реализовано; обзор кадров |
| 1 | 3 | Удачный API (`api-success-pixel`) | Sol simple | Реализовано; обзор кадров |
| 1 | 5 | Сонная улыбка (`artboard-24cla2`) | Sol simple | Реализовано; обзор кадров |
| 1 | 6 | Зелёный щит (`artboard-24cla3`) | Sol simple | Реализовано; обзор кадров |
| 1 | 11 | Знак ошибки (`artboard-2cla2`) | Sol simple | Реализовано; обзор кадров |
| 1 | 16 | Сердце идеи (`artboard-4cla`) | Sol simple | Реализовано; обзор кадров |
| 1 | 18 | Песочные часы (`artboard-4cla3`) | Sol simple | Реализовано; обзор кадров |
| 1 | 20 | Грустное сердечко (`broken-heart-sad`) | Sol simple | Реализовано; обзор кадров |
| 1 | 23 | Капли грусти (`crying-sad-reaction`) | Sol simple | Реализовано; обзор кадров |
| 1 | 30 | Зелёный шар (`examining-green-globe`) | Sol simple | Реализовано; обзор кадров |
| 1 | 36 | Древний свиток (`holding-ancient-scroll`) | Sol simple | Реализовано; обзор кадров |
| 1 | 37 | Зелёный лист (`holding-green-leaf`) | Sol simple | Реализовано; обзор кадров |
| 1 | 38 | Лампочка идеи (`idea-lightbulb-pixel`) | Sol simple | Реализовано; обзор кадров |
| 1 | 40 | Мягкое сияние (`in-glowing-aura`) | Sol simple | Реализовано; обзор кадров |
| 1 | 42 | Знак зависимости (`it-depends-sign`) | Sol simple | Реализовано; обзор кадров |
| 1 | 45 | Облачко любви (`love-cloud-pixel`) | Sol simple | Реализовано; обзор кадров |
| 1 | 54 | Чтение документа (`reading-data-document`) | Sol simple | Реализовано; обзор кадров |
| 1 | 59 | Замочек с ключом (`secure-access-padlock`) | Sol simple | Реализовано; обзор кадров |
| 1 | 61 | Крепкий сон (`sleeping-soundly`) | Sol simple | Реализовано; обзор кадров |
| 1 | 64 | Розовая карточка (`system-error-alert`) | Sol simple | Реализовано; обзор кадров |
| 1 | 67 | Пузырёк мыслей (`thinking-pixel-bubble`) | Sol simple | Реализовано; обзор кадров |
| 1 | 72 | Добро пожаловать (`welcome-pixel-banner`) | Sol simple | Реализовано; обзор кадров |
| 1 | 73 | Зелёная галочка (`with-success-checkmark`) | Sol simple | Реализовано; обзор кадров |
| 2 | 2 | Молния энергии (`angry-powerful-lightning`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 4 | Золотое торжество (`artboard-24cla`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 7 | Быстрые шаги (`artboard-25cla`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 8 | Пиксельная игра (`artboard-25cla2`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 9 | Маленький сад (`artboard-25cla3`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 10 | Зелёная диаграмма (`artboard-2cla`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 12 | Синий костюм (`artboard-2cla3`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 13 | Два пузырька (`artboard-3cla`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 14 | Цветные искры (`artboard-3cla2`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 15 | Луч света (`artboard-3cla3`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 17 | Пиксельный переполох (`artboard-4cla2`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 19 | Пирог из пикселей (`baking-steaming-pie`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 22 | Спираль мыслей (`confusion-spiral-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 24 | Блоки данных (`data-blocks-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 25 | Пиксельная отладка (`debugging-system-bugs`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 28 | Головокружительный сбой (`dizzy-failed-state`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 29 | Розовое предупреждение (`error-state-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 31 | Неудачная попытка (`failed-process-error`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 32 | Скоростной след (`fast-performance-trail`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 33 | Спокойствие в огне (`fine-under-pressure`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 34 | Пиксельная зелень (`growing-pixel-nature`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 35 | Радостный танец (`happy-pixel-celebrating`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 39 | Кольцо пикселей (`in-constant-loop`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 46 | Окошко с пикселями (`malware-detection-window`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 48 | Заметка на экране (`marking-critical-error`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 50 | Пиксельные грибочки (`nature-growth-mushrooms`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 51 | Маленькие деревца (`planting-small-trees`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 52 | Знаки логики (`processing-logic-symbols`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 53 | Аккуратные заметки (`professional-documentation-audit`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 55 | Круг обновления (`refresh-sync-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 60 | Стопка серверов (`server-stack-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 62 | Тихий ниндзя (`stealth-ninja-security`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 63 | Новый рубеж (`success-achievement-milestone`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 65 | Пиксельная панель (`system-monitoring-dashboard`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 66 | Мысли в коде (`thinking-in-code`) | Sol advanced | Реализовано; обзор кадров |
| 2 | 69 | Растущая стрелка (`trending-up-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 0 | Сердитый ноутбук (`angry-at-laptop`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 21 | Цветные кубики (`building-colorful-blocks`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 26 | Парное программирование (`developer-pair-programming-scene`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 27 | Вилка и розетка (`disconnected-plug-pixel`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 41 | Цветные серверы (`infrastructure-server-stacks`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 43 | Пиксельная лаборатория (`lab-science-testing`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 44 | Пиксельная штанга (`lifting-heavy-barbell`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 47 | Стойка из кубиков (`managing-server-rack`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 49 | Много окошек (`multi-window-workflow`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 56 | Инструменты ремонта (`repairing-with-tools`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 57 | Длинная лента (`reviewing-long-data-log`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 58 | Пиксельная ракета (`rocket-launch-success`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 68 | Точки на карте (`tracking-multiple-locations`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 70 | Настройки аватара (`user-profile-settings`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 71 | Дерево данных (`visualizing-tree-data`) | Sol advanced | Реализовано; обзор кадров |
| 3 | 74 | Пиксельный код (`writing-software-code`) | Sol advanced | Реализовано; обзор кадров |

## Дополнительные сцены Codrops

Флаг, два выброса конфетти и гантель добавлены после исходного пака как ID75–77. Отдельные временные шкалы и тесты готовы. Первое ревью Terra обнаружило и помогло устранить разрывы полотнища, резкий захват гантели и несовпадение ладоней с источником конфетти. Из прогулки взята идея оседания ладоней при посадке; траектория остаётся у физики.

Экспорт настоящих кадров:

```powershell
cargo run --manifest-path rust/Cargo.toml --example activity_frames -- rust/work/activity-review
```

## Результат проверки 0.14.0

Все 75 строк реализованы: 23 simple и 52 advanced. Дополнительно 3 showcase,
итого 78 занятий. Общий прогон: 317 тестов прошли, 1 проверка реального MSI
запускается отдельно в release workflow. Clippy без замечаний.

Terra просмотрела несколько поз каждой из 75 сцен. Для 9 выбранных сцен
дополнительно проверены оба направления и 75%/300%. Исправлены связанные
предметы, цельное полотно флага, захват и возврат гантели, начало конфетти,
нечитаемые сердечки, надписи в отражении, завершение башни и общие руки.
Полный независимый просмотр каждого кадра/масштаба не утверждается.
[Фактический объём ревью](../handoffs/terra-pack-animation-review.md).
