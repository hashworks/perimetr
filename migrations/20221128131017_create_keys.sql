CREATE TABLE shares (
    id SERIAL PRIMARY KEY NOT NULL,
    layer_uuid VARCHAR NOT NULL,
    share VARCHAR NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX shares_layer_uuid_share ON shares (layer_uuid, share);