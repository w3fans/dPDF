`page-photo.jpg` is an original dPDF test image (400×200 px): colored side panels,
sample text and a drawn dummy stroke. It contains no real signature or customer
content. The page corpus injects EXIF orientation into a copy to test phone-photo
rotation and checks that untransformed JPEG data stays byte-identical in the PDF.
PNG cases are generated with Python's standard library by `check-pages.py`.
