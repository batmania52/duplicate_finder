# Results
## 2026-04-18 개선 전

```
python3 duplicate_finder.py scan ~/nas/Nas/Sub1 --no-phash --no-vhash
시작: 2026-04-18 21:47:40
[탐색 시작] /Users/macbook/nas/Nas/Sub1
  탐색 완료: 2694개 파일 발견

[압축파일 검사] 18개 압축파일 내부 검사 중...
  압축파일 내부 항목: 75963개 추출 완료

전체 검사 대상: 78,657개 (파일 2,694 + 압축 내부 75,963)

[해시 계산] 크기 동일 후보: 71653개
  부분 해시 후보: 50127개
  중복 그룹: 16407개 발견      127
  pHash 대상 제외: 해시 중복 확인 33,720개

[저장 완료] duplicates_20260418_214740.csv

==================================================
  중복 그룹 수  : 16,407개
  중복 파일 수  : 50,127개
  낭비 용량     : 4948.8 MB (4.83 GB)
==================================================

CSV 파일을 열어서 keep 컬럼을 수정한 후 delete 명령으로 삭제하세요:
  python duplicate_finder.py delete duplicates_20260418_214740.csv

[압축파일 겹침 분석] 2개 이상 공통 파일 기준...
[저장 완료] archive_overlaps_20260418_214740.csv

압축파일 겹침 쌍: 44개
상위 5개:
  공통 12148개 | 박효진.zip ↔ 박효진1.zip
  공통 3449개 | 2018_gsretail_cvs.zip ↔ 박효진.zip
  공통 3449개 | 2018_gsretail_cvs.zip ↔ 박효진1.zip
  공통 1500개 | 박효진.zip ↔ DSCOM.tar.gz
  공통 1500개 | 박효진1.zip ↔ DSCOM.tar.gz

소요 시간: 0:02:13.890151
(.venv)
```

```
 python duplicate_finder.py scan /Users/macbook/nas/Nas/Sub3/movs /Users/macbook/nas/Nas/Sub3/Uncen /Users/macbook/nas/Nas/Sub3/Hitomi /Users/macbook/nas/Movs
시작: 2026-04-18 22:10:38
[탐색 시작] /Users/macbook/nas/Nas/Sub3/movs
  탐색 완료: 1333개 파일 발견
[탐색 시작] /Users/macbook/nas/Nas/Sub3/Uncen
  탐색 완료: 99개 파일 발견
[탐색 시작] /Users/macbook/nas/Nas/Sub3/Hitomi
  탐색 완료: 4213개 파일 발견
[탐색 시작] /Users/macbook/nas/Movs
  탐색 완료: 531개 파일 발견

전체 탐색 합계: 6,176개 파일

[압축파일 검사] 2066개 압축파일 내부 검사 중...
/Users/macbook/projects/utils-project/.venv/lib/python3.14/site-packages/PIL/Image.py:1137: UserWarning: Palette images with Transparency expressed in bytes should be converted to RGBA images
  warnings.warn(
  압축파일 내부 항목: 217604개 추출 완료      treon reward (3649455).zip417).zip 활 (decensored) (3583435).zip빛 갸루 (3650294).zip 된 모양이다 6 (2822716).zippp2868585).zip

전체 검사 대상: 223,780개 (파일 6,176 + 압축 내부 217,604)

[해시 계산] 크기 동일 후보: 57692개
  부분 해시 후보: 17810개
  중복 그룹: 8443개 발견      7810
  pHash 대상 제외: 해시 중복 확인 9,365개

[pHash 수집] 이미지 205255개 pHash 계산 중...
  pHash 수집 완료

[영상 pHash 수집] 영상 1887개 프레임 추출 중... (파일당 10프레임)
  주의: 영상 수에 따라 시간이 오래 걸릴 수 있어요
  (43/1887) Bukkake on the beautiful face of Airi Miyazaki, a super beautiful breasts F cup beauty with outstanding style that remained until the final selection of a certain famou  (422/1887) SNIS-787 Hyper High Pressure J Cup Titty Fuck Action 37 Cum Shots In Non Stop Goddess Tits Large Orgies Action RION  JavHD Jav Streaming Jav Porn  Japanese Porn Videos  (644/1887) videoplaybackupn=NLVlNL-Mz-0&signature=1A4CA3134AEAB2D91B027D53BED4781A718563D0.138F836A4F02D621805701E5CEDCF79F88A7C1F3&ipbits=0&sparams=clen%2Cdur%2Cei%2Cgir%2Cid%2C  (1034/1887) EPORNER.COM - [Q2KDhBjGj01] For 1 night and 2 days, you can have Hikaru Nagi exclusively for 10 million yen at the world's most luxurious and fabulous divine milk del  (1039/1887) EPORNER.COM - [xcpwNrnGuUZ] SS IS 870 [Decensored] Hikaru Nagi, A Luxurious Lotion Soap Girl Who Makes Her Soft J-cup Breasts Slippery And Wraps Them Around The Man -  (1382/1887) Wealthy people are studying at a high-priced appointee college, young and beautiful, beautiful girls, compensated dating girls, the services are in place, and they ar  (1597/1887) [15.07.24] Ai Uehara, Mao Hamasaki, Nanase Otoha, Maria Wakatsuki - [UMANAMI] チ一ム對抗 逆ナンパでル一レットの旅！！ in 中央線沿線編 上原亞衣 浜崎眞緖 乙葉ななせ 若   영상 pHash 수집 완료: 1638/1887개      p4아Ol 몇명이랑 할까」.mp4않아♪」.aviukashii tte Minna ga Iu kara 1.mp4).aviプルレズバトル 上原亞衣 夏目優希 內村りな [LZPL-006].avi

[저장 완료] duplicates_20260418_221038.csv

==================================================
  중복 그룹 수  : 8,443개
  중복 파일 수  : 17,808개
  낭비 용량     : 39211.3 MB (38.29 GB)
==================================================

CSV 파일을 열어서 keep 컬럼을 수정한 후 delete 명령으로 삭제하세요:
  python duplicate_finder.py delete duplicates_20260418_221038.csv

[이미지 유사도 분석] 213945개 이미지 비교 중...
  완전동일 그룹: 18613개 / 유사 그룹: 11261개
[저장 완료] image_similar_20260418_221038.csv

이미지 완전동일 그룹: 18613개 / 유사 그룹: 11261개
  (기준: exact≤0, similar≤10)

[영상 유사도 분석] 1638개 영상 비교 중...
  완전동일 그룹: 13개 / 유사 그룹: 20개
[저장 완료] video_similar_20260418_221038.csv

영상 완전동일 그룹: 13개 / 유사 그룹: 20개
  (기준: exact≤3.0, similar≤10.0, 10프레임)

[압축파일 겹침 분석] 2개 이상 공통 파일 기준...
[저장 완료] archive_overlaps_20260418_221038.csv

압축파일 겹침 쌍: 117개
상위 5개:
  공통 786개 | Tsugumomo - Alternative pages.zip ↔ [Hamada Yoshikadu, Yamamoto Yammy] Tsugumomo - Alternative pages (2811796).zip
  공통 720개 | joy ride (pixiv_770371).zip ↔ joy ride (pixiv_770371).zip
  공통 475개 | [Ndgd] NDGD (5624416) 2026.01.30 (3763866).zip ↔ [Ndgd] NDGD (5624416) 2026.01.28 (3760684).zip
  공통 331개 | 楓子.zip ↔ ◆FANBOX◆ 楓子.zip
  공통 313개 | [joy ride] Imouto de Ou-sama Game ~Goukon de Imouto to Battari de Ou-sama Game suru Hanashi~ (761877).zip ↔ Imouto de Ou-sama Game ~Goukon de Imouto to Battari de Ou-sama Game suru Hanashi~ by joy ride.zip

소요 시간: 5:42:02.810003
(.venv)
```
## 2026-04-19 개선 후
```
 python3 duplicate_finder.py scan ~/nas/Nas/Sub1 --no-phash --no-vhash
시작: 2026-04-19 09:38:08
[탐색 시작] /Users/macbook/nas/Nas/Sub1
  탐색 완료: 2694개 파일 발견

[압축파일 검사] 18개 압축파일 내부 검사 중...
  압축파일 내부 항목: 75963개 추출 완료

전체 검사 대상: 78,657개 (파일 2,694 + 압축 내부 75,963)

[해시 계산] 크기 동일 후보: 71653개
  부분 해시 계산 중... 71640/71653
  부분 해시 후보: 50127개
  전체 해시 계산 중... 50120/50127
  중복 그룹: 16407개 발견
  pHash 대상 제외: 해시 중복 확인 33,720개

[저장 완료] duplicates_20260419_093808.csv

==================================================
  중복 그룹 수  : 16,407개
  중복 파일 수  : 50,127개
  낭비 용량     : 4948.8 MB (4.83 GB)
==================================================

CSV 파일을 열어서 keep 컬럼을 수정한 후 delete 명령으로 삭제하세요:
  python duplicate_finder.py delete duplicates_20260419_093808.csv

[압축파일 겹침 분석] 2개 이상 공통 파일 기준...
[저장 완료] archive_overlaps_20260419_093808.csv

압축파일 겹침 쌍: 44개
상위 5개:
  공통 12148개 | 박효진.zip ↔ 박효진1.zip
  공통 3449개 | 2018_gsretail_cvs.zip ↔ 박효진.zip
  공통 3449개 | 2018_gsretail_cvs.zip ↔ 박효진1.zip
  공통 1500개 | 박효진.zip ↔ DSCOM.tar.gz
  공통 1500개 | 박효진1.zip ↔ DSCOM.tar.gz

소요 시간: 0:01:35.760980
(.venv)
~/projects/utils-project
❯ ls -rtl
(.venv)
~/projects/utils-project
❯ python duplicate_finder.py scan /Users/macbook/nas/Nas/Sub3/movs /Users/macbook/nas/Nas/Sub3/Uncen /Users/macbook/nas/Nas/Sub3/Hitomi /Users/macbook/nas/Movs
시작: 2026-04-19 09:40:37
[탐색 시작] /Users/macbook/nas/Nas/Sub3/movs
  탐색 완료: 1333개 파일 발견
[탐색 시작] /Users/macbook/nas/Nas/Sub3/Uncen
  탐색 완료: 99개 파일 발견
[탐색 시작] /Users/macbook/nas/Nas/Sub3/Hitomi
  탐색 완료: 4213개 파일 발견
[탐색 시작] /Users/macbook/nas/Movs
  탐색 완료: 531개 파일 발견

전체 탐색 합계: 6,176개 파일

[압축파일 검사] 2066개 압축파일 내부 검사 중...
/Users/macbook/projects/utils-project/.venv/lib/python3.14/site-packages/PIL/Image.py:1137: UserWarning: Palette images with Transparency expressed in bytes should be converted to RGBA images
  warnings.warn(
  압축파일 내부 항목: 217604개 추출 완료      ensored) (3650287).zip문화적인 성생활 (decensored) (3583435).zip빛 갸루 (3650294).zip 된 모양이다 6 (2822716).zipip2868585).zip

전체 검사 대상: 223,780개 (파일 6,176 + 압축 내부 217,604)

[해시 계산] 크기 동일 후보: 57692개
  부분 해시 계산 중... 57680/57692
  부분 해시 후보: 17810개
  전체 해시 계산 중... 17800/17810
  중복 그룹: 8443개 발견
  pHash 대상 제외: 해시 중복 확인 9,365개

[pHash 수집] 이미지 205255개 pHash 계산 중...
  pHash 수집 완료

[영상 pHash 수집] 영상 1887개 프레임 추출 중... (파일당 10프레임)
  주의: 영상 수에 따라 시간이 오래 걸릴 수 있어요
  (41/1887) Bukkake on the beautiful face of Airi Miyazaki, a super beautiful breasts F cup beauty with outstanding style that remained until the final selection of a certain famou  (431/1887) SNIS-787 Hyper High Pressure J Cup Titty Fuck Action 37 Cum Shots In Non Stop Goddess Tits Large Orgies Action RION  JavHD Jav Streaming Jav Porn  Japanese Porn Videos  (639/1887) videoplaybackupn=NLVlNL-Mz-0&signature=1A4CA3134AEAB2D91B027D53BED4781A718563D0.138F836A4F02D621805701E5CEDCF79F88A7C1F3&ipbits=0&sparams=clen%2Cdur%2Cei%2Cgir%2Cid%2C  (1032/1887) EPORNER.COM - [Q2KDhBjGj01] For 1 night and 2 days, you can have Hikaru Nagi exclusively for 10 million yen at the world's most luxurious and fabulous divine milk del  (1039/1887) EPORNER.COM - [xcpwNrnGuUZ] SS IS 870 [Decensored] Hikaru Nagi, A Luxurious Lotion Soap Girl Who Makes Her Soft J-cup Breasts Slippery And Wraps Them Around The Man -  (1378/1887) Wealthy people are studying at a high-priced appointee college, young and beautiful, beautiful girls, compensated dating girls, the services are in place, and they ar  (1595/1887) [15.07.24] Ai Uehara, Mao Hamasaki, Nanase Otoha, Maria Wakatsuki - [UMANAMI] チ一ム對抗 逆ナンパでル一レットの旅！！ in 中央線沿線編 上原亞衣 浜崎眞緖 乙葉ななせ 若   영상 pHash 수집 완료: 1637/1887개      같이 야하게 되어보자☆저질러라♪」.aviiukashii tte Minna ga Iu kara 1.mp4).aviプルレズバトル 上原亞衣 夏目優希 內村りな [LZPL-006].avi

[저장 완료] duplicates_20260419_094037.csv

==================================================
  중복 그룹 수  : 8,443개
  중복 파일 수  : 17,808개
  낭비 용량     : 39211.3 MB (38.29 GB)
==================================================

CSV 파일을 열어서 keep 컬럼을 수정한 후 delete 명령으로 삭제하세요:
  python duplicate_finder.py delete duplicates_20260419_094037.csv

[이미지 유사도 분석] 213945개 이미지 비교 중...
  완전동일 그룹: 18613개 / 유사 그룹: 11261개
[저장 완료] image_similar_20260419_094037.csv

이미지 완전동일 그룹: 18613개 / 유사 그룹: 11261개
  (기준: exact≤0, similar≤10)

[영상 유사도 분석] 1637개 영상 비교 중...
  완전동일 그룹: 13개 / 유사 그룹: 20개
[저장 완료] video_similar_20260419_094037.csv

영상 완전동일 그룹: 13개 / 유사 그룹: 20개
  (기준: exact≤3.0, similar≤10.0, 10프레임)

[압축파일 겹침 분석] 2개 이상 공통 파일 기준...
[저장 완료] archive_overlaps_20260419_094037.csv

압축파일 겹침 쌍: 117개
상위 5개:
  공통 786개 | Tsugumomo - Alternative pages.zip ↔ [Hamada Yoshikadu, Yamamoto Yammy] Tsugumomo - Alternative pages (2811796).zip
  공통 720개 | joy ride (pixiv_770371).zip ↔ joy ride (pixiv_770371).zip
  공통 475개 | [Ndgd] NDGD (5624416) 2026.01.30 (3763866).zip ↔ [Ndgd] NDGD (5624416) 2026.01.28 (3760684).zip
  공통 331개 | 楓子.zip ↔ ◆FANBOX◆ 楓子.zip
  공통 313개 | [joy ride] Imouto de Ou-sama Game ~Goukon de Imouto to Battari de Ou-sama Game suru Hanashi~ (761877).zip ↔ Imouto de Ou-sama Game ~Goukon de Imouto to Battari de Ou-sama Game suru Hanashi~ by joy ride.zip

소요 시간: 1:26:11.798425
(.venv)
```
