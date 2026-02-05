-- todo status
-- 0 = ToDo, 1 = InProgress, 2 = Done
ALTER TABLE todos ADD COLUMN status INTEGER NOT NULL DEFAULT 0;

UPDATE todos
SET status = CASE
    WHEN done == TRUE THEN 2
    ELSE 0
END;

ALTER TABLE todos DROP COLUMN done;

-- todo priority
-- 0 = Low, 1 = Medium, 2 = High
ALTER TABLE todos ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
