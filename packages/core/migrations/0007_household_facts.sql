-- Frozen development content only: no reviews, exposure receipts or learned schedules.
-- The corpus contains 60 te reo facts and 40 Rust/Linux facts, with their sources
-- and practice variants. SQLx records this migration once on fresh and existing databases.
CREATE TEMP TABLE household_people (
    member TEXT PRIMARY KEY,
    profile_id INTEGER NOT NULL UNIQUE
);
INSERT INTO household_people
SELECT 'ansam', id FROM profiles WHERE lower(trim(name)) = 'ansam' ORDER BY id LIMIT 1;
INSERT INTO household_people
SELECT 'josiah', id FROM profiles WHERE lower(trim(name)) = 'josiah' ORDER BY id LIMIT 1;

CREATE TEMP TABLE household_seed (
    member TEXT NOT NULL,
    document TEXT NOT NULL CHECK (json_valid(document)),
    fact_id INTEGER,
    is_new INTEGER NOT NULL DEFAULT 0
);
INSERT INTO household_seed(member, document)
VALUES
    ('ansam', '{
    "prompt": "What does “kai” mean in te reo Māori (noun)?",
    "answer": "Food; a meal",
    "context": "Noun sense. Kai can also be a verb meaning to eat.",
    "source": "https://maoridictionary.co.nz/search?idiom=0&keywords=kai&loan=0&phrase=0&proverb=0",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “kai” mean in te reo Māori (noun)?",
            "answer": "Food; a meal",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “food; a meal”. Include any macrons.",
            "answer": "kai",
            "choices": [],
            "explanation": "Noun sense. Kai can also be a verb meaning to eat."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “kai” as a noun?",
            "answer": "Food; a meal",
            "choices": [
                "Food; a meal",
                "Water",
                "Mountain",
                "Children"
            ],
            "explanation": "kai: food; a meal. Noun sense. Kai can also be a verb meaning to eat."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “wai” mean in te reo Māori (water sense)?",
    "answer": "Water",
    "context": "The water sense is intended here. Wai also has other senses, including a separate use in questions about who someone is.",
    "source": "https://maoridictionary.co.nz/search?keywords=wai",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “wai” mean in te reo Māori (water sense)?",
            "answer": "Water",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “water”. Include any macrons.",
            "answer": "wai",
            "choices": [],
            "explanation": "The water sense is intended here. Wai also has other senses, including a separate use in questions about who someone is."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “wai” in the water sense?",
            "answer": "Water",
            "choices": [
                "Food; a meal",
                "Water",
                "Mountain",
                "Song; chant"
            ],
            "explanation": "wai: water. The water sense is intended here. Wai also has other senses, including a separate use in questions about who someone is."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “whare” mean in te reo Māori?",
    "answer": "House; dwelling",
    "context": "A building or dwelling. Distinguish this from kāinga, which refers to a home or place of abode.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “whare” mean in te reo Māori?",
            "answer": "House; dwelling",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “house; dwelling”. Include any macrons.",
            "answer": "whare",
            "choices": [],
            "explanation": "A building or dwelling. Distinguish this from kāinga, which refers to a home or place of abode."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “whare”?",
            "answer": "House; dwelling",
            "choices": [
                "Food; a meal",
                "Children",
                "House; dwelling",
                "Song; chant"
            ],
            "explanation": "whare: house; dwelling. A building or dwelling. Distinguish this from kāinga, which refers to a home or place of abode."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “kāinga” mean in te reo Māori?",
    "answer": "Home; place of abode",
    "context": "This word refers to a home or place of abode. Retain the macron on ā.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “kāinga” mean in te reo Māori?",
            "answer": "Home; place of abode",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “home; place of abode”. Include any macrons.",
            "answer": "kāinga",
            "choices": [],
            "explanation": "This word refers to a home or place of abode. Retain the macron on ā."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “kāinga”?",
            "answer": "Home; place of abode",
            "choices": [
                "Water",
                "Love; compassion; empathy",
                "Meeting; assembly",
                "Home; place of abode"
            ],
            "explanation": "kāinga: home; place of abode. This word refers to a home or place of abode. Retain the macron on ā."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “whānau” mean in te reo Māori (noun)?",
    "answer": "Family; extended family group",
    "context": "The noun sense includes an extended family group. Whānau can also mean to be born. Retain the macron on ā.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “whānau” mean in te reo Māori (noun)?",
            "answer": "Family; extended family group",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “family; extended family group”. Include any macrons.",
            "answer": "whānau",
            "choices": [],
            "explanation": "The noun sense includes an extended family group. Whānau can also mean to be born. Retain the macron on ā."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “whānau” as a noun?",
            "answer": "Family; extended family group",
            "choices": [
                "Family; extended family group",
                "Mountain",
                "Song; chant",
                "Food; a meal"
            ],
            "explanation": "whānau: family; extended family group. The noun sense includes an extended family group. Whānau can also mean to be born. Retain the macron on ā."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “tamariki” mean in te reo Māori (noun)?",
    "answer": "Children",
    "context": "The noun is normally plural. It also has uses relating to being young.",
    "source": "https://maoridictionary.co.nz/search?keywords=tamariki",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “tamariki” mean in te reo Māori (noun)?",
            "answer": "Children",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “children”. Include any macrons.",
            "answer": "tamariki",
            "choices": [],
            "explanation": "The noun is normally plural. It also has uses relating to being young."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tamariki” as a noun?",
            "answer": "Children",
            "choices": [
                "Food; a meal",
                "Children",
                "Mountain",
                "Love; compassion; empathy"
            ],
            "explanation": "tamariki: children. The noun is normally plural. It also has uses relating to being young."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “aroha” mean in te reo Māori?",
    "answer": "Love; compassion; empathy",
    "context": "These are common meanings, rather than an exhaustive one-to-one English equivalent.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “aroha” mean in te reo Māori?",
            "answer": "Love; compassion; empathy",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “love; compassion; empathy”. Include any macrons.",
            "answer": "aroha",
            "choices": [],
            "explanation": "These are common meanings, rather than an exhaustive one-to-one English equivalent."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “aroha”?",
            "answer": "Love; compassion; empathy",
            "choices": [
                "House; dwelling",
                "Water",
                "Love; compassion; empathy",
                "Meeting; assembly"
            ],
            "explanation": "aroha: love; compassion; empathy. These are common meanings, rather than an exhaustive one-to-one English equivalent."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “hui” mean in te reo Māori?",
    "answer": "Meeting; assembly",
    "context": "A gathering or coming together.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “hui” mean in te reo Māori?",
            "answer": "Meeting; assembly",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “meeting; assembly”. Include any macrons.",
            "answer": "hui",
            "choices": [],
            "explanation": "A gathering or coming together."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “hui”?",
            "answer": "Meeting; assembly",
            "choices": [
                "Mountain",
                "Children",
                "Food; a meal",
                "Meeting; assembly"
            ],
            "explanation": "hui: meeting; assembly. A gathering or coming together."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “maunga” mean in te reo Māori?",
    "answer": "Mountain",
    "context": "Te Ara also lists the form mounga.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “maunga” mean in te reo Māori?",
            "answer": "Mountain",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “mountain”. Include any macrons.",
            "answer": "maunga",
            "choices": [],
            "explanation": "Te Ara also lists the form mounga."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “maunga”?",
            "answer": "Mountain",
            "choices": [
                "Mountain",
                "Food; a meal",
                "Love; compassion; empathy",
                "House; dwelling"
            ],
            "explanation": "maunga: mountain. Te Ara also lists the form mounga."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “whenua” mean in te reo Māori (land sense)?",
    "answer": "Land; ground; earth",
    "context": "This card asks for the land sense. Whenua can also mean afterbirth or placenta.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “whenua” mean in te reo Māori (land sense)?",
            "answer": "Land; ground; earth",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “land; ground; earth”. Include any macrons.",
            "answer": "whenua",
            "choices": [],
            "explanation": "This card asks for the land sense. Whenua can also mean afterbirth or placenta."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “whenua” in the land sense?",
            "answer": "Land; ground; earth",
            "choices": [
                "Food; a meal",
                "Land; ground; earth",
                "Song; chant",
                "Meeting; assembly"
            ],
            "explanation": "whenua: land; ground; earth. This card asks for the land sense. Whenua can also mean afterbirth or placenta."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “kōrero” mean in te reo Māori (noun)?",
    "answer": "Speech; story",
    "context": "Noun senses. Kōrero can also be a verb meaning to speak. Retain the macron on ō.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “kōrero” mean in te reo Māori (noun)?",
            "answer": "Speech; story",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “speech; story”. Include any macrons.",
            "answer": "kōrero",
            "choices": [],
            "explanation": "Noun senses. Kōrero can also be a verb meaning to speak. Retain the macron on ō."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “kōrero” as a noun?",
            "answer": "Speech; story",
            "choices": [
                "Water",
                "Mountain",
                "Speech; story",
                "Children"
            ],
            "explanation": "kōrero: speech; story. Noun senses. Kōrero can also be a verb meaning to speak. Retain the macron on ō."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "What does “waiata” mean in te reo Māori (noun)?",
    "answer": "Song; chant",
    "context": "Noun senses. Waiata can also be a verb meaning to sing or chant.",
    "source": "https://teara.govt.nz/en/glossary",
    "tags": [
        "te-reo-māori",
        "beginner",
        "vocabulary"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does “waiata” mean in te reo Māori (noun)?",
            "answer": "Song; chant",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "Give a te reo Māori word for “song; chant”. Include any macrons.",
            "answer": "waiata",
            "choices": [],
            "explanation": "Noun senses. Waiata can also be a verb meaning to sing or chant."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “waiata” as a noun?",
            "answer": "Song; chant",
            "choices": [
                "House; dwelling",
                "Food; a meal",
                "Family; extended family group",
                "Song; chant"
            ],
            "explanation": "waiata: song; chant. Noun senses. Waiata can also be a verb meaning to sing or chant."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “parāoa” mean in te reo Māori?",
    "answer": "Bread",
    "context": "The bread sense in a food context; parāoa can also refer to flour or dough.",
    "source": "https://maoridictionary.co.nz/search?keywords=paraoa",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “parāoa” mean in te reo Māori?",
            "answer": "Bread",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “bread” in te reo Māori?",
            "answer": "parāoa",
            "choices": [],
            "explanation": "The bread sense in a food context; parāoa can also refer to flour or dough."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “parāoa” in this kitchen context?",
            "answer": "Bread",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Egg"
            ],
            "explanation": "The bread sense in a food context; parāoa can also refer to flour or dough."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “miraka” mean in te reo Māori?",
    "answer": "Milk",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “miraka” mean in te reo Māori?",
            "answer": "Milk",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “milk” in te reo Māori?",
            "answer": "miraka",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “miraka” in this kitchen context?",
            "answer": "Milk",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Egg"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “tīhi” mean in te reo Māori?",
    "answer": "Cheese",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “tīhi” mean in te reo Māori?",
            "answer": "Cheese",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “cheese” in te reo Māori?",
            "answer": "tīhi",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tīhi” in this kitchen context?",
            "answer": "Cheese",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Egg"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “hēki” mean in te reo Māori?",
    "answer": "Egg",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “hēki” mean in te reo Māori?",
            "answer": "Egg",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “egg” in te reo Māori?",
            "answer": "hēki",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “hēki” in this kitchen context?",
            "answer": "Egg",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Egg"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “pata” mean in te reo Māori?",
    "answer": "Butter",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “pata” mean in te reo Māori?",
            "answer": "Butter",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “butter” in te reo Māori?",
            "answer": "pata",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “pata” in this kitchen context?",
            "answer": "Butter",
            "choices": [
                "Butter",
                "Bread",
                "Milk",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “huka” mean in te reo Māori?",
    "answer": "Sugar",
    "context": "The sugar sense in a food context.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “huka” mean in te reo Māori?",
            "answer": "Sugar",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “sugar” in te reo Māori?",
            "answer": "huka",
            "choices": [],
            "explanation": "The sugar sense in a food context."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “huka” in this kitchen context?",
            "answer": "Sugar",
            "choices": [
                "Bread",
                "Sugar",
                "Milk",
                "Cheese"
            ],
            "explanation": "The sugar sense in a food context."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “tote” mean in te reo Māori?",
    "answer": "Salt",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “tote” mean in te reo Māori?",
            "answer": "Salt",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “salt” in te reo Māori?",
            "answer": "tote",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tote” in this kitchen context?",
            "answer": "Salt",
            "choices": [
                "Bread",
                "Milk",
                "Salt",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “pepa” mean in te reo Māori?",
    "answer": "Pepper",
    "context": "The pepper sense in a food context.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “pepa” mean in te reo Māori?",
            "answer": "Pepper",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “pepper” in te reo Māori?",
            "answer": "pepa",
            "choices": [],
            "explanation": "The pepper sense in a food context."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “pepa” in this kitchen context?",
            "answer": "Pepper",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Pepper"
            ],
            "explanation": "The pepper sense in a food context."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “kawhe” mean in te reo Māori?",
    "answer": "Coffee",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “kawhe” mean in te reo Māori?",
            "answer": "Coffee",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “coffee” in te reo Māori?",
            "answer": "kawhe",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “kawhe” in this kitchen context?",
            "answer": "Coffee",
            "choices": [
                "Coffee",
                "Bread",
                "Milk",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “tī” mean in te reo Māori?",
    "answer": "Tea",
    "context": "Tea as a drink.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “tī” mean in te reo Māori?",
            "answer": "Tea",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “tea” in te reo Māori?",
            "answer": "tī",
            "choices": [],
            "explanation": "Tea as a drink."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tī” in this kitchen context?",
            "answer": "Tea",
            "choices": [
                "Bread",
                "Tea",
                "Milk",
                "Cheese"
            ],
            "explanation": "Tea as a drink."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “āporo” mean in te reo Māori?",
    "answer": "Apple",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “āporo” mean in te reo Māori?",
            "answer": "Apple",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “apple” in te reo Māori?",
            "answer": "āporo",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “āporo” in this kitchen context?",
            "answer": "Apple",
            "choices": [
                "Bread",
                "Milk",
                "Apple",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “panana” mean in te reo Māori?",
    "answer": "Banana",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “panana” mean in te reo Māori?",
            "answer": "Banana",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “banana” in te reo Māori?",
            "answer": "panana",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “panana” in this kitchen context?",
            "answer": "Banana",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Banana"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “rīwai” mean in te reo Māori?",
    "answer": "Potato",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “rīwai” mean in te reo Māori?",
            "answer": "Potato",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “potato” in te reo Māori?",
            "answer": "rīwai",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “rīwai” in this kitchen context?",
            "answer": "Potato",
            "choices": [
                "Potato",
                "Bread",
                "Milk",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “huarākau” mean in te reo Māori?",
    "answer": "Fruit",
    "context": "Also written hua rākau.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “huarākau” mean in te reo Māori?",
            "answer": "Fruit",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “fruit” in te reo Māori?",
            "answer": "huarākau",
            "choices": [],
            "explanation": "Also written hua rākau."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “huarākau” in this kitchen context?",
            "answer": "Fruit",
            "choices": [
                "Bread",
                "Fruit",
                "Milk",
                "Cheese"
            ],
            "explanation": "Also written hua rākau."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “huawhenua” mean in te reo Māori?",
    "answer": "Vegetables",
    "context": "Also written hua whenua.",
    "source": "https://kupu.maori.nz/set/Food-Time",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “huawhenua” mean in te reo Māori?",
            "answer": "Vegetables",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “vegetables” in te reo Māori?",
            "answer": "huawhenua",
            "choices": [],
            "explanation": "Also written hua whenua."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “huawhenua” in this kitchen context?",
            "answer": "Vegetables",
            "choices": [
                "Bread",
                "Milk",
                "Vegetables",
                "Cheese"
            ],
            "explanation": "Also written hua whenua."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “mīti” mean in te reo Māori?",
    "answer": "Meat",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “mīti” mean in te reo Māori?",
            "answer": "Meat",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “meat” in te reo Māori?",
            "answer": "mīti",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “mīti” in this kitchen context?",
            "answer": "Meat",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Meat"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “heihei” mean in te reo Māori?",
    "answer": "Chicken",
    "context": "Chicken, including the food sense.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “heihei” mean in te reo Māori?",
            "answer": "Chicken",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “chicken” in te reo Māori?",
            "answer": "heihei",
            "choices": [],
            "explanation": "Chicken, including the food sense."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “heihei” in this kitchen context?",
            "answer": "Chicken",
            "choices": [
                "Chicken",
                "Bread",
                "Milk",
                "Cheese"
            ],
            "explanation": "Chicken, including the food sense."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “tōhi” mean in te reo Māori?",
    "answer": "Toast",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “tōhi” mean in te reo Māori?",
            "answer": "Toast",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “toast” in te reo Māori?",
            "answer": "tōhi",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tōhi” in this kitchen context?",
            "answer": "Toast",
            "choices": [
                "Bread",
                "Toast",
                "Milk",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “tiamu” mean in te reo Māori?",
    "answer": "Jam",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “tiamu” mean in te reo Māori?",
            "answer": "Jam",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “jam” in te reo Māori?",
            "answer": "tiamu",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tiamu” in this kitchen context?",
            "answer": "Jam",
            "choices": [
                "Bread",
                "Milk",
                "Jam",
                "Cheese"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For food or drink, what does “hanawiti” mean in te reo Māori?",
    "answer": "Sandwich",
    "context": "",
    "source": "https://kupu.maori.nz/set/Food-Time",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "food"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For food or drink, what does “hanawiti” mean in te reo Māori?",
            "answer": "Sandwich",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “sandwich” in te reo Māori?",
            "answer": "hanawiti",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “hanawiti” in this kitchen context?",
            "answer": "Sandwich",
            "choices": [
                "Bread",
                "Milk",
                "Cheese",
                "Sandwich"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “kapu” mean in te reo Māori?",
    "answer": "Cup",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “kapu” mean in te reo Māori?",
            "answer": "Cup",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “cup” in te reo Māori?",
            "answer": "kapu",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “kapu” in this kitchen context?",
            "answer": "Cup",
            "choices": [
                "Cup",
                "Plate",
                "Knife",
                "Fork"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “pereti” mean in te reo Māori?",
    "answer": "Plate",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “pereti” mean in te reo Māori?",
            "answer": "Plate",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “plate” in te reo Māori?",
            "answer": "pereti",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “pereti” in this kitchen context?",
            "answer": "Plate",
            "choices": [
                "Cup",
                "Plate",
                "Knife",
                "Fork"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “naihi” mean in te reo Māori?",
    "answer": "Knife",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “naihi” mean in te reo Māori?",
            "answer": "Knife",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “knife” in te reo Māori?",
            "answer": "naihi",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “naihi” in this kitchen context?",
            "answer": "Knife",
            "choices": [
                "Cup",
                "Plate",
                "Knife",
                "Fork"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “paoka” mean in te reo Māori?",
    "answer": "Fork",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “paoka” mean in te reo Māori?",
            "answer": "Fork",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “fork” in te reo Māori?",
            "answer": "paoka",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “paoka” in this kitchen context?",
            "answer": "Fork",
            "choices": [
                "Cup",
                "Plate",
                "Knife",
                "Fork"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “pune” mean in te reo Māori?",
    "answer": "Spoon",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “pune” mean in te reo Māori?",
            "answer": "Spoon",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “spoon” in te reo Māori?",
            "answer": "pune",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “pune” in this kitchen context?",
            "answer": "Spoon",
            "choices": [
                "Spoon",
                "Cup",
                "Plate",
                "Knife"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “tēpu” mean in te reo Māori?",
    "answer": "Table",
    "context": "",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “tēpu” mean in te reo Māori?",
            "answer": "Table",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “table” in te reo Māori?",
            "answer": "tēpu",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “tēpu” in this kitchen context?",
            "answer": "Table",
            "choices": [
                "Cup",
                "Table",
                "Plate",
                "Knife"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “pouaka makariri” mean in te reo Māori?",
    "answer": "Fridge",
    "context": "",
    "source": "https://kupu.maori.nz/set/At-Home",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “pouaka makariri” mean in te reo Māori?",
            "answer": "Fridge",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “fridge” in te reo Māori?",
            "answer": "pouaka makariri",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “pouaka makariri” in this kitchen context?",
            "answer": "Fridge",
            "choices": [
                "Cup",
                "Plate",
                "Fridge",
                "Knife"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For kitchen objects, what does “umu” mean in te reo Māori?",
    "answer": "Oven",
    "context": "",
    "source": "https://kupu.maori.nz/set/Cooking",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "kitchen"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For kitchen objects, what does “umu” mean in te reo Māori?",
            "answer": "Oven",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “oven” in te reo Māori?",
            "answer": "umu",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which meaning fits “umu” in this kitchen context?",
            "answer": "Oven",
            "choices": [
                "Cup",
                "Plate",
                "Knife",
                "Oven"
            ],
            "explanation": ""
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “tunu” mean in te reo Māori?",
    "answer": "To cook; bake; roast; grill",
    "context": "An action word for cooking, including baking or roasting.",
    "source": "https://kupu.maori.nz/set/Cooking",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “tunu” mean in te reo Māori?",
            "answer": "To cook; bake; roast; grill",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to cook; bake; roast; grill” in te reo Māori?",
            "answer": "tunu",
            "choices": [],
            "explanation": "An action word for cooking, including baking or roasting."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “tunu” in this household context?",
            "answer": "To cook; bake; roast; grill",
            "choices": [
                "To cook; bake; roast; grill",
                "To cut",
                "To wash; clean",
                "To tidy; put in order"
            ],
            "explanation": "An action word for cooking, including baking or roasting."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “tapahi” mean in te reo Māori?",
    "answer": "To cut",
    "context": "For cutting something. The related tapatapahi is used for chopping or dicing.",
    "source": "https://kupu.maori.nz/set/Cooking",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “tapahi” mean in te reo Māori?",
            "answer": "To cut",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to cut” in te reo Māori?",
            "answer": "tapahi",
            "choices": [],
            "explanation": "For cutting something. The related tapatapahi is used for chopping or dicing."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “tapahi” in this household context?",
            "answer": "To cut",
            "choices": [
                "To cook; bake; roast; grill",
                "To cut",
                "To wash; clean",
                "To tidy; put in order"
            ],
            "explanation": "For cutting something. The related tapatapahi is used for chopping or dicing."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “horoi” mean in te reo Māori?",
    "answer": "To wash; clean",
    "context": "Useful for washing dishes, clothes, or surfaces.",
    "source": "https://maoridictionary.co.nz/search?keywords=horoi",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “horoi” mean in te reo Māori?",
            "answer": "To wash; clean",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to wash; clean” in te reo Māori?",
            "answer": "horoi",
            "choices": [],
            "explanation": "Useful for washing dishes, clothes, or surfaces."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “horoi” in this household context?",
            "answer": "To wash; clean",
            "choices": [
                "To cook; bake; roast; grill",
                "To cut",
                "To wash; clean",
                "To drink"
            ],
            "explanation": "Useful for washing dishes, clothes, or surfaces."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “whakapai” mean in te reo Māori?",
    "answer": "To tidy; put in order",
    "context": "The household tidying sense. Whakapai can also mean repair or bless.",
    "source": "https://kupu.maori.nz/kupu/whakapai%28ngia-tia%29",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “whakapai” mean in te reo Māori?",
            "answer": "To tidy; put in order",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to tidy; put in order” in te reo Māori?",
            "answer": "whakapai",
            "choices": [],
            "explanation": "The household tidying sense. Whakapai can also mean repair or bless."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “whakapai” in this household context?",
            "answer": "To tidy; put in order",
            "choices": [
                "To cook; bake; roast; grill",
                "To cut",
                "To drink",
                "To tidy; put in order"
            ],
            "explanation": "The household tidying sense. Whakapai can also mean repair or bless."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “inu” mean in te reo Māori?",
    "answer": "To drink",
    "context": "An action word for drinking.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/mi/he-reo-tupu-he-reo-ora---kai-unit-3/5637257087.p?domain=kauwhatareo.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “inu” mean in te reo Māori?",
            "answer": "To drink",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to drink” in te reo Māori?",
            "answer": "inu",
            "choices": [],
            "explanation": "An action word for drinking."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “inu” in this household context?",
            "answer": "To drink",
            "choices": [
                "To drink",
                "To cook; bake; roast; grill",
                "To cut",
                "To wash; clean"
            ],
            "explanation": "An action word for drinking."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "For household actions, what does “whakairi” mean in te reo Māori?",
    "answer": "To hang something up",
    "context": "Useful when talking about hanging up washing.",
    "source": "https://kupu.maori.nz/set/Washing-clothes",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "actions"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For household actions, what does “whakairi” mean in te reo Māori?",
            "answer": "To hang something up",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "In a household context, how do you say “to hang something up” in te reo Māori?",
            "answer": "whakairi",
            "choices": [],
            "explanation": "Useful when talking about hanging up washing."
        },
        {
            "kind": "multiple_choice",
            "prompt": "Which action fits “whakairi” in this household context?",
            "answer": "To hang something up",
            "choices": [
                "To cook; bake; roast; grill",
                "To hang something up",
                "To cut",
                "To wash; clean"
            ],
            "explanation": "Useful when talking about hanging up washing."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Are you hungry?” to your partner in te reo Māori?",
    "answer": "Kei te hiakai koe?",
    "context": "Koe addresses one person.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/ka-mau-te-wehi-unit-17---kei-te-reri-k-rua/5637168599.p?domain=newzealandcurriculum.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Are you hungry?” to your partner in te reo Māori?",
            "answer": "Kei te hiakai koe?",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Kei te hiakai koe?” mean?",
            "answer": "Are you hungry?",
            "choices": [],
            "explanation": "Koe addresses one person."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner whether they are hungry. What could you say in te reo Māori?",
            "answer": "Kei te hiakai koe?",
            "choices": [],
            "explanation": "Are you hungry? Koe addresses one person."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Are you thirsty?” to your partner in te reo Māori?",
    "answer": "Kei te hiainu koe?",
    "context": "Koe addresses one person.",
    "source": "https://tewhariki.tahurangi.education.govt.nz/ka-mau-te-wehi-unit-17---kei-te-reri-k-rua/5637168599.p?domain=newzealandcurriculum.tahurangi.education.govt.nz",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Are you thirsty?” to your partner in te reo Māori?",
            "answer": "Kei te hiainu koe?",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Kei te hiainu koe?” mean?",
            "answer": "Are you thirsty?",
            "choices": [],
            "explanation": "Koe addresses one person."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner whether they would like a drink. What could you say in te reo Māori?",
            "answer": "Kei te hiainu koe?",
            "choices": [],
            "explanation": "Are you thirsty? Koe addresses one person."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Would you like a cup of tea?” to your partner in te reo Māori?",
    "answer": "He kapu tī māu?",
    "context": "An offer to one person; māu means for you in this offer.",
    "source": "https://maoridictionary.co.nz/search?keywords=ti",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Would you like a cup of tea?” to your partner in te reo Māori?",
            "answer": "He kapu tī māu?",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “He kapu tī māu?” mean?",
            "answer": "Would you like a cup of tea?",
            "choices": [],
            "explanation": "An offer to one person; māu means for you in this offer."
        },
        {
            "kind": "apply",
            "prompt": "Offer your partner a cup of tea. What could you say in te reo Māori?",
            "answer": "He kapu tī māu?",
            "choices": [],
            "explanation": "Would you like a cup of tea? An offer to one person; māu means for you in this offer."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “The food is ready!” to your partner in te reo Māori?",
    "answer": "Kua rite te kai!",
    "context": "Use when the meal is ready.",
    "source": "https://www.reomaori.co.nz/bbq-raumati",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “The food is ready!” to your partner in te reo Māori?",
            "answer": "Kua rite te kai!",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Kua rite te kai!” mean?",
            "answer": "The food is ready!",
            "choices": [],
            "explanation": "Use when the meal is ready."
        },
        {
            "kind": "apply",
            "prompt": "Tell your partner that the food is ready. What could you say in te reo Māori?",
            "answer": "Kua rite te kai!",
            "choices": [],
            "explanation": "The food is ready! Use when the meal is ready."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Please pass me the butter.” to your partner in te reo Māori?",
    "answer": "Tēnā, homai te pata.",
    "context": "Uses the published Tēnā, homai te ____ pattern with pata, butter.",
    "source": "https://www.reomaori.co.nz/bbq-raumati",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Please pass me the butter.” to your partner in te reo Māori?",
            "answer": "Tēnā, homai te pata.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Tēnā, homai te pata.” mean?",
            "answer": "Please pass me the butter.",
            "choices": [],
            "explanation": "Uses the published Tēnā, homai te ____ pattern with pata, butter."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner to pass you the butter. What could you say in te reo Māori?",
            "answer": "Tēnā, homai te pata.",
            "choices": [],
            "explanation": "Please pass me the butter. Uses the published Tēnā, homai te ____ pattern with pata, butter."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Please set the table.” to your partner in te reo Māori?",
    "answer": "Tēnā, whakaritea te tēpu.",
    "context": "A request to prepare the table.",
    "source": "https://www.reomaori.co.nz/bbq-raumati",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Please set the table.” to your partner in te reo Māori?",
            "answer": "Tēnā, whakaritea te tēpu.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Tēnā, whakaritea te tēpu.” mean?",
            "answer": "Please set the table.",
            "choices": [],
            "explanation": "A request to prepare the table."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner to set the table for the meal. What could you say in te reo Māori?",
            "answer": "Tēnā, whakaritea te tēpu.",
            "choices": [],
            "explanation": "Please set the table. A request to prepare the table."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “This is delicious!” to your partner in te reo Māori?",
    "answer": "Te reka hoki!",
    "context": "An expression of enjoyment of the food.",
    "source": "https://www.reomaori.co.nz/bbq-raumati",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “This is delicious!” to your partner in te reo Māori?",
            "answer": "Te reka hoki!",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Te reka hoki!” mean?",
            "answer": "This is delicious!",
            "choices": [],
            "explanation": "An expression of enjoyment of the food."
        },
        {
            "kind": "apply",
            "prompt": "Tell your partner that the food tastes delicious. What could you say in te reo Māori?",
            "answer": "Te reka hoki!",
            "choices": [],
            "explanation": "This is delicious! An expression of enjoyment of the food."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “What would you like?” to your partner in te reo Māori?",
    "answer": "He aha māu?",
    "context": "Useful while offering food or a drink.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “What would you like?” to your partner in te reo Māori?",
            "answer": "He aha māu?",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “He aha māu?” mean?",
            "answer": "What would you like?",
            "choices": [],
            "explanation": "Useful while offering food or a drink."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner what they would like. What could you say in te reo Māori?",
            "answer": "He aha māu?",
            "choices": [],
            "explanation": "What would you like? Useful while offering food or a drink."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Please wash the dishes.” to your partner in te reo Māori?",
    "answer": "Tēnā, horoia ngā rīhi.",
    "context": "A request using horoia, the passive command form of horoi.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Please wash the dishes.” to your partner in te reo Māori?",
            "answer": "Tēnā, horoia ngā rīhi.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Tēnā, horoia ngā rīhi.” mean?",
            "answer": "Please wash the dishes.",
            "choices": [],
            "explanation": "A request using horoia, the passive command form of horoi."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner to wash the dishes. What could you say in te reo Māori?",
            "answer": "Tēnā, horoia ngā rīhi.",
            "choices": [],
            "explanation": "Please wash the dishes. A request using horoia, the passive command form of horoi."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Dry the dishes.” to your partner in te reo Māori?",
    "answer": "Whakamarokehia ngā rīhi.",
    "context": "A direct instruction; choose a tone that suits your partner.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Dry the dishes.” to your partner in te reo Māori?",
            "answer": "Whakamarokehia ngā rīhi.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Whakamarokehia ngā rīhi.” mean?",
            "answer": "Dry the dishes.",
            "choices": [],
            "explanation": "A direct instruction; choose a tone that suits your partner."
        },
        {
            "kind": "apply",
            "prompt": "Ask for the dishes to be dried after washing. What could you say in te reo Māori?",
            "answer": "Whakamarokehia ngā rīhi.",
            "choices": [],
            "explanation": "Dry the dishes. A direct instruction; choose a tone that suits your partner."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Fetch the milk.” to your partner in te reo Māori?",
    "answer": "Tīkina mai te miraka.",
    "context": "Mai points towards the speaker.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Fetch the milk.” to your partner in te reo Māori?",
            "answer": "Tīkina mai te miraka.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Tīkina mai te miraka.” mean?",
            "answer": "Fetch the milk.",
            "choices": [],
            "explanation": "Mai points towards the speaker."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner to bring you the milk. What could you say in te reo Māori?",
            "answer": "Tīkina mai te miraka.",
            "choices": [],
            "explanation": "Fetch the milk. Mai points towards the speaker."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Put this in the fridge.” to your partner in te reo Māori?",
    "answer": "Hoatu tēnei ki te pouaka makariri.",
    "context": "Tēnei refers to this item.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Put this in the fridge.” to your partner in te reo Māori?",
            "answer": "Hoatu tēnei ki te pouaka makariri.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Hoatu tēnei ki te pouaka makariri.” mean?",
            "answer": "Put this in the fridge.",
            "choices": [],
            "explanation": "Tēnei refers to this item."
        },
        {
            "kind": "apply",
            "prompt": "Ask your partner to put the item you are indicating in the fridge. What could you say in te reo Māori?",
            "answer": "Hoatu tēnei ki te pouaka makariri.",
            "choices": [],
            "explanation": "Put this in the fridge. Tēnei refers to this item."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “Come and help.” to your partner in te reo Māori?",
    "answer": "Haere mai ki te āwhina.",
    "context": "An invitation to come and assist.",
    "source": "https://www.rangitane.org.nz/wp-content/uploads/2018/11/TRRK-Rack-Cards.pdf",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “Come and help.” to your partner in te reo Māori?",
            "answer": "Haere mai ki te āwhina.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Haere mai ki te āwhina.” mean?",
            "answer": "Come and help.",
            "choices": [],
            "explanation": "An invitation to come and assist."
        },
        {
            "kind": "apply",
            "prompt": "Invite your partner to come and help with a household job. What could you say in te reo Māori?",
            "answer": "Haere mai ki te āwhina.",
            "choices": [],
            "explanation": "Come and help. An invitation to come and assist."
        }
    ]
}'),
    ('ansam', '{
    "prompt": "How would you say “I will help you.” to your partner in te reo Māori?",
    "answer": "Māku koe e āwhina.",
    "context": "Māku identifies the speaker as the person who will help.",
    "source": "https://kupu.maori.nz/kupu/%C4%81whina",
    "tags": [
        "te-reo-māori",
        "household",
        "beginner",
        "partner-phrases"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How would you say “I will help you.” to your partner in te reo Māori?",
            "answer": "Māku koe e āwhina.",
            "choices": [],
            "explanation": ""
        },
        {
            "kind": "flashcard",
            "prompt": "What does “Māku koe e āwhina.” mean?",
            "answer": "I will help you.",
            "choices": [],
            "explanation": "Māku identifies the speaker as the person who will help."
        },
        {
            "kind": "apply",
            "prompt": "Offer to help your partner with a household job. What could you say in te reo Māori?",
            "answer": "Māku koe e āwhina.",
            "choices": [],
            "explanation": "I will help you. Māku identifies the speaker as the person who will help."
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Are Rust variable bindings mutable by default, and how do you allow reassignment?",
    "answer": "Bindings are immutable by default. Use let mut to allow reassignment.",
    "context": "Example: let mut count = 0; count += 1;. A new let binding can shadow an earlier binding, even with a different type.",
    "source": "https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html",
    "tags": [
        "programming",
        "rust",
        "fundamentals"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Are Rust variable bindings mutable by default, and how do you allow reassignment?",
            "answer": "Bindings are immutable by default. Use let mut to allow reassignment.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "After let b = a; where a is a String, can you still use a?",
    "answer": "No. Ownership of the String moves into b, so a can no longer be used as that value.",
    "context": "String is not Copy. By contrast, assigning an i32 copies its value; a.clone() explicitly creates another owned String.",
    "source": "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html",
    "tags": [
        "programming",
        "rust",
        "ownership"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "After let b = a; where a is a String, can you still use a?",
            "answer": "No. Ownership of the String moves into b, so a can no longer be used as that value.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In safe Rust, may a value have a mutable borrow while shared borrows to it are still in use?",
    "answer": "No. For overlapping use, the same value can have shared borrows or one exclusive mutable borrow, not both.",
    "context": "A shared borrow can end at its last use, allowing a later mutable borrow in the same block.",
    "source": "https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html",
    "tags": [
        "programming",
        "rust",
        "borrowing"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In safe Rust, may a value have a mutable borrow while shared borrows to it are still in use?",
            "answer": "No. For overlapping use, the same value can have shared borrows or one exclusive mutable borrow, not both.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What is the ownership difference between String and &str in Rust?",
    "answer": "String owns growable UTF-8 text; &str borrows a UTF-8 string slice stored elsewhere.",
    "context": "A function accepting &str can usually borrow text from either a String or a string literal without taking ownership.",
    "source": "https://doc.rust-lang.org/book/ch08-02-strings.html",
    "tags": [
        "programming",
        "rust",
        "strings",
        "ownership"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What is the ownership difference between String and &str in Rust?",
            "answer": "String owns growable UTF-8 text; &str borrows a UTF-8 string slice stored elsewhere.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Which Rust enum represents an optional value, and what are its two variants?",
    "answer": "Option<T>: Some(value) represents a present value and None represents absence.",
    "context": "Handle absence explicitly, for example with match, if let, or an appropriate Option method.",
    "source": "https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html",
    "tags": [
        "programming",
        "rust",
        "types",
        "option"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Which Rust enum represents an optional value, and what are its two variants?",
            "answer": "Option<T>: Some(value) represents a present value and None represents absence.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "How does Rust Result<T, E> represent success and recoverable failure?",
    "answer": "Ok(value) carries a successful T; Err(error) carries an error E.",
    "context": "For example, opening a file returns a Result that can be handled or propagated by the caller.",
    "source": "https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html",
    "tags": [
        "programming",
        "rust",
        "error-handling"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How does Rust Result<T, E> represent success and recoverable failure?",
            "answer": "Ok(value) carries a successful T; Err(error) carries an error E.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does the ? operator do when applied to a Result in a compatible Result-returning function?",
    "answer": "It extracts an Ok value and continues, or returns an Err early from the enclosing function, converting the error when required.",
    "context": "Example: let text = std::fs::read_to_string(path)?;. The enclosing return type must support the propagated error; compatible Result error conversion uses From.",
    "source": "https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html",
    "tags": [
        "programming",
        "rust",
        "error-handling"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does the ? operator do when applied to a Result in a compatible Result-returning function?",
            "answer": "It extracts an Ok value and continues, or returns an Err early from the enclosing function, converting the error when required.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does it mean that a Rust match expression must be exhaustive?",
    "answer": "Its arms must cover every possible value of the matched type.",
    "context": "List all relevant cases or use an appropriate catch-all pattern such as _. The compiler rejects missing cases.",
    "source": "https://doc.rust-lang.org/book/ch06-02-match.html",
    "tags": [
        "programming",
        "rust",
        "pattern-matching"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does it mean that a Rust match expression must be exhaustive?",
            "answer": "Its arms must cover every possible value of the matched type.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Why does constructing a Rust iterator chain with map not necessarily run its closure?",
    "answer": "Iterators are lazy: the chain must be driven by something that consumes items, such as collect, sum, or a for loop.",
    "context": "Example: values.iter().map(|x| x + 1).collect::<Vec<_>>() evaluates the mapping while collecting the results.",
    "source": "https://doc.rust-lang.org/book/ch13-02-iterators.html",
    "tags": [
        "programming",
        "rust",
        "iterators"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Why does constructing a Rust iterator chain with map not necessarily run its closure?",
            "answer": "Iterators are lazy: the chain must be driven by something that consumes items, such as collect, sum, or a for loop.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "For an owned Vec<T>, how do iter(), iter_mut(), and into_iter() differ?",
    "answer": "iter() yields &T, iter_mut() yields &mut T, and into_iter() consumes the vector and yields owned T values.",
    "context": "This distinction is scoped to these methods on an owned Vec<T>; IntoIterator implementations for references can yield borrowed items.",
    "source": "https://doc.rust-lang.org/book/ch13-02-iterators.html",
    "tags": [
        "programming",
        "rust",
        "iterators",
        "ownership"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For an owned Vec<T>, how do iter(), iter_mut(), and into_iter() differ?",
            "answer": "iter() yields &T, iter_mut() yields &mut T, and into_iter() consumes the vector and yields owned T values.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does a trait bound such as T: Display require in Rust?",
    "answer": "T must implement Display, so generic code can rely on the behaviour that trait provides.",
    "context": "Traits describe shared behaviour. A bound constrains which types a generic function or type accepts.",
    "source": "https://doc.rust-lang.org/book/ch10-02-traits.html",
    "tags": [
        "programming",
        "rust",
        "traits",
        "generics"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does a trait bound such as T: Display require in Rust?",
            "answer": "T must implement Display, so generic code can rely on the behaviour that trait provides.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "How does cargo check differ from cargo build?",
    "answer": "cargo check checks packages without the final code-generation step; cargo build produces compiled artifacts.",
    "context": "check is generally faster, but diagnostics that arise only during code generation can still require a full build.",
    "source": "https://doc.rust-lang.org/cargo/commands/cargo-check.html",
    "tags": [
        "programming",
        "rust",
        "cargo",
        "cli"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How does cargo check differ from cargo build?",
            "answer": "cargo check checks packages without the final code-generation step; cargo build produces compiled artifacts.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Do explicit lifetime annotations make borrowed Rust data live longer?",
    "answer": "No. They describe relationships between reference lifetimes; they do not extend the lifetime of the underlying data.",
    "context": "For example, a shared lifetime parameter can relate an output reference to input references, letting the compiler check validity.",
    "source": "https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html",
    "tags": [
        "programming",
        "rust",
        "lifetimes",
        "borrowing"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Do explicit lifetime annotations make borrowed Rust data live longer?",
            "answer": "No. They describe relationships between reference lifetimes; they do not extend the lifetime of the underlying data.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What is the distinction between the Rust Send and Sync marker traits?",
    "answer": "Send allows ownership of values to move between threads. Sync means shared references can cross threads safely: T is Sync when &T is Send.",
    "context": "Moving ownership and sharing references are different properties. Rc<T> implements neither trait.",
    "source": "https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html",
    "tags": [
        "programming",
        "rust",
        "concurrency"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What is the distinction between the Rust Send and Sync marker traits?",
            "answer": "Send allows ownership of values to move between threads. Sync means shared references can cross threads safely: T is Sync when &T is Send.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Why is -- present in cargo test -- --no-capture?",
    "answer": "Arguments before -- go to Cargo; arguments after it go to the test binary. --no-capture disables the test harness output capture.",
    "context": "This is argument forwarding to another program, rather than a Cargo option named --no-capture.",
    "source": "https://doc.rust-lang.org/rustc/tests/index.html",
    "tags": [
        "programming",
        "rust",
        "cargo",
        "cli",
        "testing"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Why is -- present in cargo test -- --no-capture?",
            "answer": "Arguments before -- go to Cargo; arguments after it go to the test binary. --no-capture disables the test harness output capture.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "How do Cargo --locked and --offline differ, and what does --frozen combine?",
    "answer": "--locked requires the existing Cargo.lock without changes. --offline forbids network access. --frozen enables both.",
    "context": "--locked can still download locked dependencies. --offline can fail if required dependencies are not cached; it does not itself require an unchanged lockfile.",
    "source": "https://doc.rust-lang.org/cargo/commands/cargo-build.html",
    "tags": [
        "programming",
        "rust",
        "cargo",
        "cli",
        "dependencies"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How do Cargo --locked and --offline differ, and what does --frozen combine?",
            "answer": "--locked requires the existing Cargo.lock without changes. --offline forbids network access. --frozen enables both.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In GNU commands such as ls, what does a standalone -- do?",
    "answer": "It ends option parsing; following arguments are treated as operands even if their names begin with a dash.",
    "context": "Example: ls -- -report lists the file named -report. This convention is command-dependent; it is not a universal rule for every CLI.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/Common-options.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "options",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In GNU commands such as ls, what does a standalone -- do?",
            "answer": "It ends option parsing; following arguments are treated as operands even if their names begin with a dash.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does GNU ls -l add to a directory listing?",
    "answer": "A long listing with file type and permissions, link count, owner, group, size, and timestamp.",
    "context": "Example: ls -l notes.txt shows metadata for that file. It does not recursively list directories.",
    "source": "https://manpages.debian.org/trixie/coreutils/ls.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "ls",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does GNU ls -l add to a directory listing?",
            "answer": "A long listing with file type and permissions, link count, owner, group, size, and timestamp.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What is the difference between GNU ls -a and ls -A?",
    "answer": "-a includes dot-prefixed names, including . and ..; -A includes hidden names but omits the implied . and .. entries.",
    "context": "Example: ls -A shows .git without the two dot-directory entries.",
    "source": "https://manpages.debian.org/trixie/coreutils/ls.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "ls",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What is the difference between GNU ls -a and ls -A?",
            "answer": "-a includes dot-prefixed names, including . and ..; -A includes hidden names but omits the implied . and .. entries.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does -h mean in GNU ls -lh?",
    "answer": "It scales displayed file sizes into readable units based on powers of 1024, such as K, M, and G.",
    "context": "-l requests the long format. Here -h means human-readable sizes, not help; --si uses powers of 1000.",
    "source": "https://manpages.debian.org/trixie/coreutils/ls.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "ls",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does -h mean in GNU ls -lh?",
            "answer": "It scales displayed file sizes into readable units based on powers of 1024, such as K, M, and G.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In cp -R source/ destination/, which argument is an option and which are operands?",
    "answer": "-R is the recursive-copy option. source/ and destination/ are the source and destination operands.",
    "context": "GNU cp -R copies directories and their contents recursively. Recursive copying alone is not archive-mode metadata preservation.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/cp-invocation.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "cp",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In cp -R source/ destination/, which argument is an option and which are operands?",
            "answer": "-R is the recursive-copy option. source/ and destination/ are the source and destination operands.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does GNU cp -a do beyond an ordinary recursive copy?",
    "answer": "Archive mode copies recursively while preserving links and as many file attributes as possible; it is equivalent to -dR --preserve=all.",
    "context": "Example: cp -a project/ backup/. Preservation depends on filesystem support and permissions; it is not a guarantee of a live database-consistent backup.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/cp-invocation.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "cp",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does GNU cp -a do beyond an ordinary recursive copy?",
            "answer": "Archive mode copies recursively while preserving links and as many file attributes as possible; it is equivalent to -dR --preserve=all.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does GNU mv -i request when a destination file already exists?",
    "answer": "It prompts before overwriting that destination file.",
    "context": "Example: mv -i draft.txt final.txt. Among -i, -f, and -n, the last specified option takes effect.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/mv-invocation.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "mv",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does GNU mv -i request when a destination file already exists?",
            "answer": "It prompts before overwriting that destination file.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In GNU rm, how do -r and -f differ?",
    "answer": "-r removes directories and their contents recursively. -f suppresses prompts and ignores missing files or missing operands.",
    "context": "-f does not itself enable recursion. rm deletes rather than sending files to a desktop trash folder.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/rm-invocation.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "rm",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In GNU rm, how do -r and -f differ?",
            "answer": "-r removes directories and their contents recursively. -f suppresses prompts and ignores missing files or missing operands.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does mkdir -p projects/keepsake/data do?",
    "answer": "It creates missing parent directories and the final directory, and accepts an already-existing directory.",
    "context": "-p is --parents in GNU mkdir. An existing non-directory component can still make the command fail.",
    "source": "https://www.gnu.org/s/coreutils/manual/html_node/mkdir-invocation.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "mkdir",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does mkdir -p projects/keepsake/data do?",
            "answer": "It creates missing parent directories and the final directory, and accepts an already-existing directory.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does chmod u+x script.sh change?",
    "answer": "It adds execute permission for the file owner without removing the other existing permission bits.",
    "context": "u means owner, + adds a permission, and x means execute for a regular file. u+x is a symbolic mode operand, not a dash-prefixed option.",
    "source": "https://manpages.debian.org/trixie/coreutils/chmod.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "chmod",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does chmod u+x script.sh change?",
            "answer": "It adds execute permission for the file owner without removing the other existing permission bits.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does grep -n add to matching lines?",
    "answer": "It prefixes each matching line with its 1-based line number within the input file.",
    "context": "Example: grep -n ''error'' app.log.",
    "source": "https://www.gnu.org/s/grep/manual/grep.html#Output-Line-Prefix-Control",
    "tags": [
        "linux",
        "cli",
        "flags",
        "grep",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does grep -n add to matching lines?",
            "answer": "It prefixes each matching line with its 1-based line number within the input file.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does grep -i change about matching?",
    "answer": "It ignores case distinctions between the pattern and the input.",
    "context": "Example: grep -i ''error'' app.log also matches ERROR. Case handling can depend on the locale.",
    "source": "https://www.gnu.org/s/grep/manual/grep.html#Matching-Control",
    "tags": [
        "linux",
        "cli",
        "flags",
        "grep",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does grep -i change about matching?",
            "answer": "It ignores case distinctions between the pattern and the input.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "How do grep -F and grep -E interpret patterns differently?",
    "answer": "-F treats patterns as fixed strings; -E treats them as extended regular expressions.",
    "context": "grep -F ''a.b'' notes.txt treats the dot literally. With -E, a dot is a regular-expression metacharacter.",
    "source": "https://www.gnu.org/s/grep/manual/grep.html#grep-Programs",
    "tags": [
        "linux",
        "cli",
        "flags",
        "grep",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How do grep -F and grep -E interpret patterns differently?",
            "answer": "-F treats patterns as fixed strings; -E treats them as extended regular expressions.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In GNU grep, how do -r and -R differ when recursion encounters symbolic links?",
    "answer": "-r follows symlinks supplied as command-line operands but skips symlinks found during recursion. -R follows all symlinks.",
    "context": "Example: grep -r ''TODO'' src. The capital letter changes traversal behaviour.",
    "source": "https://www.gnu.org/s/grep/manual/grep.html#File-and-Directory-Selection",
    "tags": [
        "linux",
        "cli",
        "flags",
        "grep",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In GNU grep, how do -r and -R differ when recursion encounters symbolic links?",
            "answer": "-r follows symlinks supplied as command-line operands but skips symlinks found during recursion. -R follows all symlinks.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does find . -type f select?",
    "answer": "Regular files in the search tree rooted at the current directory.",
    "context": "-type f excludes directories. GNU find uses -P by default, so it does not normally follow symbolic links.",
    "source": "https://manpages.debian.org/trixie/findutils/find.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "find",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does find . -type f select?",
            "answer": "Regular files in the search tree rooted at the current directory.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "Why should the pattern be quoted in find . -name ''*.log''?",
    "answer": "Quoting keeps the shell from expanding *.log before find receives it, so find can apply the name pattern throughout its search.",
    "context": "-name matches the base name. It can match names of directories too; add -type f when only regular files are wanted.",
    "source": "https://manpages.debian.org/trixie/findutils/find.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "find",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "Why should the pattern be quoted in find . -name ''*.log''?",
            "answer": "Quoting keeps the shell from expanding *.log before find receives it, so find can apply the name pattern throughout its search.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does GNU find . -maxdepth 1 include?",
    "answer": "The starting directory at depth 0 and its immediate entries at depth 1, without descending into deeper directories.",
    "context": "Add -type f to select only immediate regular files. -maxdepth is not a POSIX find option.",
    "source": "https://www.gnu.org/software/findutils/manual/html_node/find_html/Directories.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "find",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does GNU find . -maxdepth 1 include?",
            "answer": "The starting directory at depth 0 and its immediate entries at depth 1, without descending into deeper directories.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In GNU tar, what operations do -c, -x, and -t select?",
    "answer": "-c creates an archive, -x extracts one, and -t lists its members. Choose one primary operation.",
    "context": "Examples: tar -cf backup.tar notes/ creates; tar -tf backup.tar lists without extracting.",
    "source": "https://manpages.debian.org/trixie/tar/tar.1.en.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "tar",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In GNU tar, what operations do -c, -x, and -t select?",
            "answer": "-c creates an archive, -x extracts one, and -t lists its members. Choose one primary operation.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What do the flags mean in tar -xzf backup.tar.gz?",
    "answer": "-x extracts, -z uses gzip decompression, and -f takes backup.tar.gz as the archive filename.",
    "context": "The archive path is the argument to -f, not an extraction directory. GNU tar uses -C directory to change the extraction location.",
    "source": "https://www.gnu.org/software/tar/manual/tar.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "tar",
        "gnu"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What do the flags mean in tar -xzf backup.tar.gz?",
            "answer": "-x extracts, -z uses gzip decompression, and -f takes backup.tar.gz as the archive filename.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "For HTTP, how do curl -I and curl -i differ?",
    "answer": "-I makes a HEAD request. -i includes response headers alongside the normal response data.",
    "context": "HEAD requests no response body. A server can handle HEAD differently from GET.",
    "source": "https://curl.se/docs/manpage.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "curl"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "For HTTP, how do curl -I and curl -i differ?",
            "answer": "-I makes a HEAD request. -i includes response headers alongside the normal response data.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "What does curl -L do for an HTTP redirect?",
    "answer": "It follows a redirect to the location supplied by the server.",
    "context": "Without -L, curl does not normally follow HTTP redirects.",
    "source": "https://curl.se/docs/manpage.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "curl"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "What does curl -L do for an HTTP redirect?",
            "answer": "It follows a redirect to the location supplied by the server.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "How do curl -o local.bin URL and curl -O URL choose the output filename?",
    "answer": "-o uses the local filename you specify; -O derives it from the filename part of the URL.",
    "context": "-O does not use a Content-Disposition filename unless combined with -J.",
    "source": "https://curl.se/docs/manpage.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "curl"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "How do curl -o local.bin URL and curl -O URL choose the output filename?",
            "answer": "-o uses the local filename you specify; -O derives it from the filename part of the URL.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In Bash, how do > and >> differ when redirecting standard output to a regular file?",
    "answer": "> creates or truncates the file; >> creates it if needed and appends instead of truncating.",
    "context": "Example: command >> output.log. These are shell operators, not flags passed to command. Bash noclobber can prevent > from replacing an existing file.",
    "source": "https://www.gnu.org/s/bash/manual/html_node/Redirections.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "bash"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In Bash, how do > and >> differ when redirecting standard output to a regular file?",
            "answer": "> creates or truncates the file; >> creates it if needed and appends instead of truncating.",
            "choices": [],
            "explanation": ""
        }
    ]
}'),
    ('josiah', '{
    "prompt": "In Bash, what does command 2> errors.log redirect?",
    "answer": "Standard error, file descriptor 2, to errors.log. Standard output is not redirected by this operator.",
    "context": "For a regular file, 2> creates or truncates it; 2>> appends. The 2 must be adjacent to >.",
    "source": "https://www.gnu.org/s/bash/manual/html_node/Redirections.html",
    "tags": [
        "linux",
        "cli",
        "flags",
        "bash"
    ],
    "practices": [
        {
            "kind": "flashcard",
            "prompt": "In Bash, what does command 2> errors.log redirect?",
            "answer": "Standard error, file descriptor 2, to errors.log. Standard output is not redirected by this operator.",
            "choices": [],
            "explanation": ""
        }
    ]
}');

-- Move existing household content in place, keeping IDs and all learning evidence.
-- Other profiles' libraries are never reassigned. A prompt and source identify a
-- seed fact; an edited answer, context or tag list is retained rather than replaced.
CREATE TEMP TABLE household_transfers (
    fact_id INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL
);
INSERT INTO household_transfers
SELECT f.id, (SELECT profile_id FROM household_people WHERE member = 'ansam')
FROM facts f
WHERE f.profile_id = (SELECT profile_id FROM household_people WHERE member = 'josiah')
  AND NOT EXISTS (
      SELECT 1 FROM fact_shares share
      WHERE share.accepted_fact_id = f.id AND share.status = 'accepted'
  )
  AND (
      EXISTS (
          SELECT 1 FROM json_each(f.tags) tag
          WHERE lower(replace(replace(trim(tag.value), '_', '-'), ' ', '-'))
              IN ('te-reo', 'te-reo-maori', 'te-reo-māori')
      )
      OR EXISTS (
          SELECT 1 FROM household_seed seed
          WHERE seed.member = 'ansam'
            AND f.prompt = json_extract(seed.document, '$.prompt')
            AND f.source = json_extract(seed.document, '$.source')
      )
  );
INSERT OR IGNORE INTO household_transfers
SELECT f.id, owner.profile_id
FROM household_seed seed
JOIN household_people owner ON owner.member = seed.member
JOIN facts f ON f.prompt = json_extract(seed.document, '$.prompt')
    AND f.source = json_extract(seed.document, '$.source')
WHERE f.profile_id IN (SELECT profile_id FROM household_people)
  AND f.profile_id <> owner.profile_id
  AND NOT EXISTS (
      SELECT 1 FROM fact_shares share
      WHERE share.accepted_fact_id = f.id AND share.status = 'accepted'
  );

-- Ownership is immutable during normal operation. Only this migration may move
-- a fact and its existing reviews/events together; restore every guard below.
DROP TRIGGER facts_owner_immutable;
DROP TRIGGER reviews_owner_immutable;
DROP TRIGGER events_owner_immutable;
DROP TRIGGER batches_owner_immutable;

UPDATE facts
SET profile_id = (SELECT profile_id FROM household_transfers WHERE fact_id = facts.id)
WHERE id IN (SELECT fact_id FROM household_transfers);
UPDATE reviews
SET profile_id = (SELECT profile_id FROM facts WHERE id = reviews.fact_id)
WHERE fact_id IN (SELECT fact_id FROM household_transfers);
UPDATE study_events
SET profile_id = (
    SELECT f.profile_id FROM learning_targets t JOIN facts f ON f.id = t.fact_id
    WHERE t.id = study_events.target_id
)
WHERE target_id IN (
    SELECT id FROM learning_targets WHERE fact_id IN (SELECT fact_id FROM household_transfers)
);

-- Receipts containing one owner's facts follow that owner. A mixed historical
-- receipt retains its original UUID and JSON; owner validation prevents replay
-- across the split. Individual exposure events and cooldowns remain intact.
UPDATE study_exposure_batches AS batch
SET profile_id = (
    SELECT min(f.profile_id)
    FROM json_each(batch.request_json, '$.fact_ids') item
    JOIN facts f ON f.id = item.value
)
WHERE json_array_length(batch.request_json, '$.fact_ids') > 0
  AND (
      SELECT count(*) FROM json_each(batch.request_json, '$.fact_ids') item
      JOIN facts f ON f.id = item.value
  ) = json_array_length(batch.request_json, '$.fact_ids')
  AND (
      SELECT count(DISTINCT f.profile_id)
      FROM json_each(batch.request_json, '$.fact_ids') item
      JOIN facts f ON f.id = item.value
  ) = 1;

CREATE TRIGGER facts_owner_immutable BEFORE UPDATE OF profile_id ON facts
WHEN NEW.profile_id IS NOT OLD.profile_id
BEGIN SELECT RAISE(ABORT, 'fact owner is immutable'); END;
CREATE TRIGGER reviews_owner_immutable BEFORE UPDATE OF profile_id, fact_id ON reviews
WHEN NEW.profile_id IS NOT OLD.profile_id OR NEW.fact_id IS NOT OLD.fact_id
BEGIN SELECT RAISE(ABORT, 'review owner is immutable'); END;
CREATE TRIGGER events_owner_immutable BEFORE UPDATE OF profile_id, target_id ON study_events
WHEN NEW.profile_id IS NOT OLD.profile_id OR NEW.target_id IS NOT OLD.target_id
BEGIN SELECT RAISE(ABORT, 'event owner is immutable'); END;
CREATE TRIGGER batches_owner_immutable BEFORE UPDATE OF profile_id ON study_exposure_batches
WHEN NEW.profile_id IS NOT OLD.profile_id
BEGIN SELECT RAISE(ABORT, 'exposure owner is immutable'); END;

-- Seed only missing facts; reopening or upgrading an existing dev library does
-- not duplicate its content or overwrite its wording and progress.
UPDATE household_seed AS seed
SET fact_id = (
    SELECT min(f.id) FROM facts f
    JOIN household_people owner ON owner.profile_id = f.profile_id
    WHERE owner.member = seed.member
      AND f.prompt = json_extract(seed.document, '$.prompt')
      AND f.source = json_extract(seed.document, '$.source')
);
UPDATE household_seed SET is_new = 1 WHERE fact_id IS NULL;

INSERT INTO facts(profile_id, prompt, answer, context, source, tags)
SELECT owner.profile_id,
    json_extract(seed.document, '$.prompt'), json_extract(seed.document, '$.answer'),
    json_extract(seed.document, '$.context'), json_extract(seed.document, '$.source'),
    json_extract(seed.document, '$.tags')
FROM household_seed seed
JOIN household_people owner ON owner.member = seed.member
WHERE seed.is_new = 1;
UPDATE household_seed AS seed
SET fact_id = (
    SELECT min(f.id) FROM facts f
    JOIN household_people owner ON owner.profile_id = f.profile_id
    WHERE owner.member = seed.member
      AND f.prompt = json_extract(seed.document, '$.prompt')
      AND f.source = json_extract(seed.document, '$.source')
);

INSERT INTO practices(fact_id, kind, prompt, answer, choices, explanation)
SELECT seed.fact_id,
    json_extract(item.value, '$.kind'), json_extract(item.value, '$.prompt'),
    json_extract(item.value, '$.answer'), json_extract(item.value, '$.choices'),
    json_extract(item.value, '$.explanation')
FROM household_seed seed, json_each(seed.document, '$.practices') item
WHERE seed.is_new = 1 AND NOT EXISTS (
    SELECT 1 FROM practices p WHERE p.fact_id = seed.fact_id
      AND p.kind = json_extract(item.value, '$.kind')
      AND p.prompt = json_extract(item.value, '$.prompt')
      AND p.answer = json_extract(item.value, '$.answer')
      AND json(p.choices) = json(json_extract(item.value, '$.choices'))
      AND p.explanation = json_extract(item.value, '$.explanation')
);
INSERT INTO practices(fact_id, kind, prompt, answer)
SELECT f.id, 'flashcard', f.prompt, f.answer
FROM facts f JOIN household_seed seed ON seed.fact_id = f.id
WHERE seed.is_new = 1 AND NOT EXISTS (
    SELECT 1 FROM practices p WHERE p.fact_id = f.id
      AND p.kind = 'flashcard' AND p.prompt = f.prompt AND p.answer = f.answer
);

INSERT INTO learning_targets(fact_id, content_revision, prompt, answer)
SELECT f.id,
    coalesce((SELECT max(content_revision) FROM learning_targets WHERE fact_id = f.id), 0) + 1,
    f.prompt, f.answer
FROM facts f JOIN household_seed seed ON seed.fact_id = f.id
WHERE seed.is_new = 1
  AND NOT EXISTS (SELECT 1 FROM learning_targets WHERE fact_id = f.id AND active = 1);
INSERT INTO target_practices(practice_id, target_id, qualifying)
SELECT p.id, t.id,
    p.kind = 'flashcard' AND p.prompt = t.prompt AND p.answer = t.answer
FROM practices p JOIN learning_targets t ON t.fact_id = p.fact_id AND t.active = 1
JOIN household_seed seed ON seed.fact_id = p.fact_id
WHERE seed.is_new = 1
  AND NOT EXISTS (SELECT 1 FROM target_practices WHERE practice_id = p.id);
INSERT INTO schedule_states(target_id, phase, version, state_json)
SELECT t.id, 'new', 1, json_object(
    'target_id', t.id, 'fact_id', t.fact_id, 'content_revision', t.content_revision,
    'phase', 'new', 'due_at', NULL, 'last_review_at', NULL, 'stability', NULL,
    'difficulty', NULL, 'lapses', 0, 'version', 1, 'introduced_at', NULL, 'blocked_until', NULL
)
FROM learning_targets t JOIN household_seed seed ON seed.fact_id = t.fact_id
WHERE seed.is_new = 1 AND t.active = 1
  AND NOT EXISTS (SELECT 1 FROM schedule_states WHERE target_id = t.id);

-- Freeze current content into question-only offers. Josiah explicitly accepts
-- independent copies with fresh schedules; no sender history is copied.
CREATE TEMP TABLE household_offers AS
SELECT f.id AS fact_id, f.prompt AS question, json_object(
    'prompt', f.prompt, 'answer', f.answer, 'context', f.context, 'source', f.source,
    'tags', json(f.tags), 'practices', json((
        SELECT json_group_array(json_object(
            'kind', p.kind, 'prompt', p.prompt, 'answer', p.answer,
            'choices', json(p.choices), 'explanation', p.explanation
        ))
        FROM (SELECT * FROM practices WHERE fact_id = f.id ORDER BY id) p
    ))
) AS snapshot_json
FROM facts f
WHERE f.profile_id = (SELECT profile_id FROM household_people WHERE member = 'ansam')
  AND (
      EXISTS (SELECT 1 FROM household_seed seed WHERE seed.fact_id = f.id AND seed.member = 'ansam')
      OR EXISTS (
          SELECT 1 FROM json_each(f.tags) tag
          WHERE lower(replace(replace(trim(tag.value), '_', '-'), ' ', '-'))
              IN ('te-reo', 'te-reo-maori', 'te-reo-māori')
      )
  );
INSERT INTO fact_shares(sender_id, recipient_id, source_fact_id, snapshot_json, fingerprint, question, status)
SELECT sender.profile_id, recipient.profile_id, offer.fact_id,
    offer.snapshot_json, offer.snapshot_json, offer.question, 'pending'
FROM household_offers offer
JOIN household_people sender ON sender.member = 'ansam'
JOIN household_people recipient ON recipient.member = 'josiah'
WHERE NOT EXISTS (
    SELECT 1 FROM fact_shares share
    WHERE share.sender_id = sender.profile_id AND share.recipient_id = recipient.profile_id
      AND share.source_fact_id = offer.fact_id AND share.fingerprint = offer.snapshot_json
);

DROP TABLE household_offers;
DROP TABLE household_transfers;
DROP TABLE household_seed;
DROP TABLE household_people;
