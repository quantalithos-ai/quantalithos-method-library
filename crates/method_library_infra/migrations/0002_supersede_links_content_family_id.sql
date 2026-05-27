alter table supersede_links
    add column if not exists content_family_id text;

update supersede_links as supersede
set content_family_id = contents.content_family_id
from method_contents as contents
where supersede.new_content_id = contents.content_id
  and supersede.content_family_id is null;

alter table supersede_links
    alter column content_family_id set not null;

create index if not exists supersede_links_content_family_idx
    on supersede_links (content_family_id);
