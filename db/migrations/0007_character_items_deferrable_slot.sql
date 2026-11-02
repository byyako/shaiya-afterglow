-- Swapping two items is one UPDATE that moves each row into the other's slot. A plain primary key is checked row by row and would
-- refuse the first row; DEFERRABLE INITIALLY IMMEDIATE checks it at the end of the statement instead.
ALTER TABLE character_items DROP CONSTRAINT character_items_pkey;
ALTER TABLE character_items ADD CONSTRAINT character_items_pkey PRIMARY KEY (character_id, bag, slot) DEFERRABLE INITIALLY IMMEDIATE;
